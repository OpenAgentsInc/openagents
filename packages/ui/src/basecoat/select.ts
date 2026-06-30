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

export type SelectFormat = 'value' | 'object'

export type SelectOption = Readonly<{
  value: string
  label: string
  disabled?: boolean
}>

export type SelectModel = Readonly<{
  options: ReadonlyArray<SelectOption>
  multiple: boolean
  open: boolean
  selectedValues: ReadonlyArray<string>
  activeValue: string | null
  placeholder: string
  closeOnSelect: boolean
  format: SelectFormat
}>

export type SelectInit = Readonly<{
  options: ReadonlyArray<SelectOption>
  multiple?: boolean
  open?: boolean
  value?: string | ReadonlyArray<string> | null
  defaultValue?: string | ReadonlyArray<string> | null
  activeValue?: string | null
  placeholder?: string
  closeOnSelect?: boolean
  format?: SelectFormat
}>

export type SelectMessage =
  | Readonly<{ _tag: 'SelectOpened' }>
  | Readonly<{ _tag: 'SelectClosed' }>
  | Readonly<{ _tag: 'SelectToggled' }>
  | Readonly<{ _tag: 'SelectFocused'; value: string }>
  | Readonly<{ _tag: 'SelectSelected'; value: string }>
  | Readonly<{ _tag: 'SelectDeselected'; value: string }>
  | Readonly<{ _tag: 'SelectKeyDown'; key: string }>

export type SelectProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: SelectModel
  children: BasecoatChildren
  name?: string
  id?: string
  listboxId?: string
  disabled?: boolean
  ariaLabel?: string
  labelledBy?: string
  describedBy?: string
  toMessage?: (message: SelectMessage) => Message
}>

export type SelectTriggerProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: SelectModel
  children?: BasecoatChildren
  controlsId?: string
  disabled?: boolean
  ariaLabel?: string
  labelledBy?: string
  describedBy?: string
  toMessage?: (message: SelectMessage) => Message
}>

export type SelectPopoverProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: SelectModel
  children: BasecoatChildren
}>

export type SelectListboxProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: SelectModel
  children: BasecoatChildren
  id?: string
  emptyText?: string
  toMessage?: (message: SelectMessage) => Message
}>

export type SelectOptionProps<Message> = BasecoatAttrs<Message> & Readonly<{
  option: SelectOption
  model: SelectModel
  id?: string
  children?: BasecoatChildren
  toMessage?: (message: SelectMessage) => Message
}>

export type SelectHiddenInputProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: SelectModel
  name?: string
}>

export type SelectViewProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: SelectModel
  options?: ReadonlyArray<SelectOption>
  name?: string
  id?: string
  listboxId?: string
  emptyText?: string
  disabled?: boolean
  ariaLabel?: string
  labelledBy?: string
  describedBy?: string
  toMessage: (message: SelectMessage) => Message
}>

const selectRoot = basecoatClass('select')
const mutedPlaceholder = basecoatClass('text-muted-foreground')
const activeOption = basecoatClass('active')

const optionId = (listboxId: string, value: string): string =>
  `${listboxId}-${value.replace(/[^a-zA-Z0-9_-]+/g, '-') || 'option'}`

const enabledOptions = (model: SelectModel): ReadonlyArray<SelectOption> =>
  model.options.filter(option => option.disabled !== true)

const findOption = (
  model: SelectModel,
  value: string,
): SelectOption | undefined =>
  model.options.find(option => option.value === value)

const findEnabledOption = (
  model: SelectModel,
  value: string,
): SelectOption | undefined =>
  enabledOptions(model).find(option => option.value === value)

const normalizeValues = (
  options: ReadonlyArray<SelectOption>,
  multiple: boolean,
  value: string | ReadonlyArray<string> | null | undefined,
): ReadonlyArray<string> => {
  const enabled = new Set(options.filter(option => option.disabled !== true).map(option => option.value))
  const values = Array.isArray(value)
    ? value
    : value === undefined || value === null || value === ''
      ? []
      : [value]
  const unique = values
    .map(String)
    .filter((candidate, index, all) => enabled.has(candidate) && all.indexOf(candidate) === index)

  return multiple ? unique : unique.slice(0, 1)
}

export const selectInit = (input: SelectInit): SelectModel => {
  const multiple = input.multiple === true
  const selectedValues = normalizeValues(
    input.options,
    multiple,
    input.value ?? input.defaultValue,
  )
  const activeValue =
    input.activeValue !== undefined && input.activeValue !== null
      ? findEnabledOption({
          options: input.options,
          multiple,
          open: input.open === true,
          selectedValues,
          activeValue: null,
          placeholder: input.placeholder ?? '',
          closeOnSelect: input.closeOnSelect === true,
          format: input.format ?? 'value',
        }, input.activeValue)?.value ?? null
      : selectedValues[0] ?? enabledOptions({
          options: input.options,
          multiple,
          open: input.open === true,
          selectedValues,
          activeValue: null,
          placeholder: input.placeholder ?? '',
          closeOnSelect: input.closeOnSelect === true,
          format: input.format ?? 'value',
        })[0]?.value ?? null

  return {
    options: input.options,
    multiple,
    open: input.open === true,
    selectedValues,
    activeValue,
    placeholder: input.placeholder ?? '',
    closeOnSelect: input.closeOnSelect === true,
    format: input.format ?? 'value',
  }
}

const activeOrSelectedValue = (model: SelectModel): string | null =>
  model.selectedValues.find(value => findEnabledOption(model, value) !== undefined) ??
  enabledOptions(model)[0]?.value ??
  null

const openSelect = (model: SelectModel): SelectModel => ({
  ...model,
  open: true,
  activeValue: model.activeValue === null || findEnabledOption(model, model.activeValue) === undefined
    ? activeOrSelectedValue(model)
    : model.activeValue,
})

const closeSelect = (model: SelectModel): SelectModel => ({
  ...model,
  open: false,
  activeValue: null,
})

const selectSingleValue = (
  model: SelectModel,
  value: string,
): SelectModel => {
  const option = findEnabledOption(model, value)
  if (option === undefined) {
    return model
  }

  return {
    ...model,
    selectedValues: option.value === '' && model.placeholder.length > 0 ? [] : [option.value],
    activeValue: option.value,
    open: false,
  }
}

const toggleMultipleValue = (
  model: SelectModel,
  value: string,
): SelectModel => {
  const option = findEnabledOption(model, value)
  if (option === undefined) {
    return model
  }

  const selectedValues = model.selectedValues.includes(option.value)
    ? model.selectedValues.filter(candidate => candidate !== option.value)
    : [...model.selectedValues, option.value]

  return {
    ...model,
    selectedValues,
    activeValue: option.value,
    open: model.closeOnSelect ? false : model.open,
  }
}

const moveActiveValue = (
  model: SelectModel,
  offset: number,
): SelectModel => {
  const options = enabledOptions(model)
  if (options.length === 0) {
    return model
  }

  const index = model.activeValue === null
    ? -1
    : options.findIndex(option => option.value === model.activeValue)
  const nextIndex = Math.min(
    options.length - 1,
    Math.max(0, index === -1 ? 0 : index + offset),
  )

  return { ...model, activeValue: options[nextIndex]?.value ?? null }
}

export const selectUpdate = (
  model: SelectModel,
  message: SelectMessage,
): SelectModel => {
  switch (message._tag) {
    case 'SelectOpened':
      return openSelect(model)
    case 'SelectClosed':
      return closeSelect(model)
    case 'SelectToggled':
      return model.open ? closeSelect(model) : openSelect(model)
    case 'SelectFocused':
      return findEnabledOption(model, message.value) === undefined
        ? model
        : { ...model, activeValue: message.value }
    case 'SelectSelected':
      return model.multiple
        ? toggleMultipleValue(model, message.value)
        : selectSingleValue(model, message.value)
    case 'SelectDeselected':
      return model.multiple && model.selectedValues.includes(message.value)
        ? {
            ...model,
            selectedValues: model.selectedValues.filter(value => value !== message.value),
            activeValue: findOption(model, message.value)?.value ?? model.activeValue,
          }
        : model
    case 'SelectKeyDown':
      if (!model.open) {
        switch (message.key) {
          case 'ArrowDown':
          case 'ArrowUp':
          case 'Home':
          case 'End':
          case 'Enter':
          case ' ':
          case 'Spacebar':
            return openSelect(model)
          default:
            return model
        }
      }

      switch (message.key) {
        case 'Escape':
          return closeSelect(model)
        case 'ArrowDown':
          return moveActiveValue(model, 1)
        case 'ArrowUp':
          return moveActiveValue(model, -1)
        case 'Home':
          return { ...model, activeValue: enabledOptions(model)[0]?.value ?? null }
        case 'End': {
          const options = enabledOptions(model)
          return { ...model, activeValue: options[options.length - 1]?.value ?? null }
        }
        case 'Enter':
        case ' ':
        case 'Spacebar':
          return model.activeValue === null
            ? model
            : selectUpdate(model, {
                _tag: 'SelectSelected',
                value: model.activeValue,
              })
        default:
          return model
      }
  }
}

const selectedOptions = (model: SelectModel): ReadonlyArray<SelectOption> =>
  model.options.filter(option => model.selectedValues.includes(option.value))

const selectedLabel = (model: SelectModel): string =>
  selectedOptions(model).map(option => option.label).join(', ')

export const selectValue = (model: SelectModel): string | ReadonlyArray<string> =>
  model.multiple ? model.selectedValues : model.selectedValues[0] ?? ''

export const selectSelected = (
  model: SelectModel,
): SelectOption | ReadonlyArray<SelectOption> | null =>
  model.multiple ? selectedOptions(model) : selectedOptions(model)[0] ?? null

export const selectSerializedValue = (model: SelectModel): string => {
  const selected = selectedOptions(model).map(option => ({
    value: option.value,
    label: option.label,
  }))

  if (model.format === 'object') {
    return JSON.stringify(model.multiple ? selected : selected[0] ?? null)
  }

  const values = selected.map(option => option.value)
  return model.multiple ? JSON.stringify(values) : values[0] ?? ''
}

const optionalStringAttr = <Message>(
  value: string | undefined,
  attr: (value: string) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> =>
  value === undefined ? [] : [attr(value)]

const mappedAttr = <Message>(
  toMessage: ((message: SelectMessage) => Message) | undefined,
  message: SelectMessage,
  attr: (message: Message) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> =>
  toMessage === undefined ? [] : [attr(toMessage(message))]

const handledSelectKeys = new Set([
  'ArrowDown',
  'ArrowUp',
  'Home',
  'End',
  'Enter',
  ' ',
  'Spacebar',
  'Escape',
])

const mappedKeydownAttr = <Message>(
  toMessage: ((message: SelectMessage) => Message) | undefined,
): ReadonlyArray<Attribute<Message>> => {
  if (toMessage === undefined) {
    return []
  }

  const h = html<Message>()

  return [
    h.OnKeyDownPreventDefault(key =>
      handledSelectKeys.has(key)
        ? Option.some(toMessage({ _tag: 'SelectKeyDown', key }))
        : Option.none(),
    ),
  ]
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

export const selectHiddenInput = <Message>(
  input: SelectHiddenInputProps<Message>,
): Html => {
  const h = html<Message>()

  return h.input([
    ...basecoatAttrs<Message>(input),
    h.Type('hidden'),
    h.Value(selectSerializedValue(input.model)),
    ...optionalStringAttr<Message>(input.name, h.Name),
  ])
}

export const selectTrigger = <Message>(
  input: SelectTriggerProps<Message>,
): Html => {
  const h = html<Message>()
  const label = selectedLabel(input.model)
  const showingPlaceholder = label.length === 0
  const activeId =
    input.controlsId !== undefined && input.model.activeValue !== null
      ? optionId(input.controlsId, input.model.activeValue)
      : undefined

  return h.button(
    [
      ...basecoatAttrs<Message>(input),
      h.Type('button'),
      h.Role('combobox'),
      h.AriaHasPopup('listbox'),
      h.AriaExpanded(input.model.open),
      ...(input.controlsId === undefined ? [] : [h.AriaControls(input.controlsId)]),
      ...optionalStringAttr<Message>(activeId, h.AriaActiveDescendant),
      ...optionalStringAttr<Message>(input.ariaLabel, h.AriaLabel),
      ...optionalStringAttr<Message>(input.labelledBy, h.AriaLabelledBy),
      ...optionalStringAttr<Message>(input.describedBy, h.AriaDescribedBy),
      ...(input.disabled === true ? [h.Disabled(true)] : []),
      ...mappedAttr<Message>(
        input.toMessage,
        { _tag: 'SelectToggled' },
        h.OnClick,
      ),
      ...mappedKeydownAttr<Message>(input.toMessage),
    ],
    [
      h.span(
        [
          ...basecoatAttrs<Message>(
            {},
            showingPlaceholder ? mutedPlaceholder : null,
          ),
        ],
        input.children ?? [showingPlaceholder ? input.model.placeholder : label],
      ),
      chevronDown<Message>(),
    ],
  )
}

export const selectPopover = <Message>(
  input: SelectPopoverProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input),
      h.DataAttribute('popover', ''),
      h.AriaHidden(!input.model.open),
    ],
    input.children,
  )
}

export const selectListbox = <Message>(
  input: SelectListboxProps<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input),
      h.Role('listbox'),
      ...(input.id === undefined ? [] : [h.Id(input.id)]),
      ...(input.model.multiple ? [h.AriaMultiSelectable(true)] : []),
      ...dataAttr<Message>('empty', input.emptyText),
      ...mappedKeydownAttr<Message>(input.toMessage),
    ],
    input.children,
  )
}

export const selectOption = <Message>(
  input: SelectOptionProps<Message>,
): Html => {
  const h = html<Message>()
  const selected = input.model.selectedValues.includes(input.option.value)
  const active = input.model.activeValue === input.option.value

  return h.div(
    [
      ...basecoatAttrs<Message>(input, active ? activeOption : null),
      h.Role('option'),
      h.DataAttribute('value', input.option.value),
      h.DataAttribute('label', input.option.label),
      ...(input.id === undefined ? [] : [h.Id(input.id)]),
      ...(selected ? [h.AriaSelected(true)] : []),
      ...(input.option.disabled === true ? [h.AriaDisabled(true)] : []),
      ...mappedAttr<Message>(
        input.toMessage,
        { _tag: 'SelectFocused', value: input.option.value },
        h.OnFocus,
      ),
      ...(input.option.disabled === true
        ? []
        : mappedAttr<Message>(
            input.toMessage,
            { _tag: 'SelectSelected', value: input.option.value },
            h.OnClick,
          )),
    ],
    input.children ?? [input.option.label],
  )
}

export const select = <Message>(input: SelectProps<Message>): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...basecoatAttrs<Message>(input, selectRoot),
      ...dataAttr<Message>('placeholder', input.model.placeholder.length > 0 ? input.model.placeholder : undefined),
      ...dataAttr<Message>('format', input.model.format === 'object' ? 'object' : undefined),
      ...dataAttr<Message>('close-on-select', input.model.closeOnSelect ? 'true' : undefined),
      ...(input.id === undefined ? [] : [h.Id(input.id)]),
    ],
    [
      selectHiddenInput<Message>({
        model: input.model,
        ...(input.name === undefined ? {} : { name: input.name }),
      }),
      ...input.children,
    ],
  )
}

export const selectView = <Message>(
  input: SelectViewProps<Message>,
): Html => {
  const listboxId = input.listboxId ?? 'select-listbox'
  const options = input.options ?? input.model.options

  return select<Message>({
    model: input.model,
    children: [
      selectTrigger<Message>({
        model: input.model,
        controlsId: listboxId,
        toMessage: input.toMessage,
        ...(input.disabled === undefined ? {} : { disabled: input.disabled }),
        ...(input.ariaLabel === undefined ? {} : { ariaLabel: input.ariaLabel }),
        ...(input.labelledBy === undefined ? {} : { labelledBy: input.labelledBy }),
        ...(input.describedBy === undefined ? {} : { describedBy: input.describedBy }),
      }),
      selectPopover<Message>({
        model: input.model,
        children: [
          selectListbox<Message>({
            model: input.model,
            id: listboxId,
            toMessage: input.toMessage,
            ...(input.emptyText === undefined ? {} : { emptyText: input.emptyText }),
            children: options.map(option =>
              selectOption<Message>({
                option,
                model: input.model,
                id: optionId(listboxId, option.value),
                toMessage: input.toMessage,
              }),
            ),
          }),
        ],
      }),
    ],
    ...(input.name === undefined ? {} : { name: input.name }),
    ...(input.id === undefined ? {} : { id: input.id }),
    ...(input.attrs === undefined ? {} : { attrs: input.attrs }),
    ...(input.className === undefined ? {} : { className: input.className }),
  })
}
