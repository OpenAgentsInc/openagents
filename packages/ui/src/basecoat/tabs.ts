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

export type TabsOrientation = 'horizontal' | 'vertical'

export type TabsVariant = 'default' | 'line'

export type TabsItem<Message> = Readonly<{
  value: string
  tab: BasecoatChildren
  panel?: BasecoatChildren
  id?: string
  panelId?: string
  disabled?: boolean
  ariaDisabled?: boolean
  tabAttrs?: ReadonlyArray<Attribute<Message>>
  panelAttrs?: ReadonlyArray<Attribute<Message>>
}>

export type TabsModel = Readonly<{
  selectedValue: string
  focusedValue?: string
}>

export type TabsMessage =
  | Readonly<{ _tag: 'SelectTab'; value: string }>
  | Readonly<{ _tag: 'FocusTab'; value: string }>
  | Readonly<{ _tag: 'BlurTab' }>
  | Readonly<{ _tag: 'KeyDown'; value: string; key: string }>
  | Readonly<{ _tag: 'RefreshTabs' }>

export type TabsInitialInput<Message> = Readonly<{
  items: ReadonlyArray<TabsItem<Message>>
  defaultValue?: string
}>

export type TabsUpdateInput<Message> = Readonly<{
  items: ReadonlyArray<TabsItem<Message>>
  orientation?: TabsOrientation
}>

export type TabsProps<Message> = BasecoatAttrs<Message> & Readonly<{
  id?: string
  items: ReadonlyArray<TabsItem<Message>>
  model: TabsModel
  orientation?: TabsOrientation
  variant?: TabsVariant
  ariaLabel?: string
  tablistAttrs?: ReadonlyArray<Attribute<Message>>
  toMessage?: (message: TabsMessage) => Message
}>

const tabsRoot = basecoatClass('tabs')

const defaultTabsId = 'tabs'

export const tabsItemDisabled = <Message>(
  item: TabsItem<Message>,
): boolean => item.disabled === true || item.ariaDisabled === true

const enabledItems = <Message>(
  items: ReadonlyArray<TabsItem<Message>>,
): ReadonlyArray<TabsItem<Message>> =>
  items.filter(item => !tabsItemDisabled(item))

const selectableItemByValue = <Message>(
  items: ReadonlyArray<TabsItem<Message>>,
  value: string,
): TabsItem<Message> | undefined => {
  const item = items.find(item => item.value === value)
  return item === undefined || tabsItemDisabled(item) ? undefined : item
}

const firstSelectableValue = <Message>(
  items: ReadonlyArray<TabsItem<Message>>,
): string => enabledItems(items)[0]?.value ?? items[0]?.value ?? ''

export const tabsSelectedValue = <Message>(
  items: ReadonlyArray<TabsItem<Message>>,
  model: TabsModel,
): string => {
  const selected = items.find(item =>
    item.value === model.selectedValue && !tabsItemDisabled(item)
  )

  return selected?.value ?? firstSelectableValue(items)
}

export const tabsInitialModel = <Message>(
  input: TabsInitialInput<Message>,
): TabsModel => ({
  selectedValue:
    input.defaultValue === undefined
      ? firstSelectableValue(input.items)
      : tabsSelectedValue(input.items, { selectedValue: input.defaultValue }),
})

export const tabsKeyboardTarget = <Message>(
  input: TabsUpdateInput<Message>,
  currentValue: string,
  key: string,
): string | undefined => {
  const selectableItems = enabledItems(input.items)
  const currentIndex = selectableItems.findIndex(item => item.value === currentValue)

  if (selectableItems.length === 0 || currentIndex === -1) {
    return undefined
  }

  const orientation = input.orientation ?? 'horizontal'

  if (key === 'Home') {
    return selectableItems[0]?.value
  }

  if (key === 'End') {
    return selectableItems[selectableItems.length - 1]?.value
  }

  if (key === 'ArrowRight' && orientation === 'horizontal') {
    return selectableItems[(currentIndex + 1) % selectableItems.length]?.value
  }

  if (key === 'ArrowLeft' && orientation === 'horizontal') {
    return selectableItems[
      (currentIndex - 1 + selectableItems.length) % selectableItems.length
    ]?.value
  }

  if (key === 'ArrowDown' && orientation === 'vertical') {
    return selectableItems[(currentIndex + 1) % selectableItems.length]?.value
  }

  if (key === 'ArrowUp' && orientation === 'vertical') {
    return selectableItems[
      (currentIndex - 1 + selectableItems.length) % selectableItems.length
    ]?.value
  }

  return undefined
}

export const tabsUpdate = <Message>(
  input: TabsUpdateInput<Message>,
  model: TabsModel,
  message: TabsMessage,
): TabsModel => {
  const selectedValue = tabsSelectedValue(input.items, model)

  switch (message._tag) {
    case 'SelectTab':
      return selectableItemByValue(input.items, message.value) === undefined
        ? { ...model, selectedValue }
        : { ...model, selectedValue: message.value }
    case 'FocusTab':
      return selectableItemByValue(input.items, message.value) === undefined
        ? { ...model, selectedValue }
        : { ...model, selectedValue, focusedValue: message.value }
    case 'BlurTab':
      return { selectedValue }
    case 'KeyDown': {
      const targetValue = tabsKeyboardTarget(input, message.value, message.key)
      return targetValue === undefined
        ? { ...model, selectedValue }
        : {
            ...model,
            selectedValue: targetValue,
            focusedValue: targetValue,
          }
    }
    case 'RefreshTabs':
      return { ...model, selectedValue }
  }
}

const tabIdFor = <Message>(
  rootId: string,
  item: TabsItem<Message>,
  index: number,
): string => item.id ?? `${rootId}-tab-${index + 1}`

const panelIdFor = <Message>(
  rootId: string,
  item: TabsItem<Message>,
  index: number,
): string => item.panelId ?? `${rootId}-panel-${index + 1}`

const eventAttrs = <Message>(
  input: TabsProps<Message>,
  item: TabsItem<Message>,
): ReadonlyArray<Attribute<Message>> => {
  if (input.toMessage === undefined || tabsItemDisabled(item)) {
    return []
  }

  const toMessage = input.toMessage

  return [
    html<Message>().OnClick(toMessage({ _tag: 'SelectTab', value: item.value })),
    html<Message>().OnFocus(toMessage({ _tag: 'FocusTab', value: item.value })),
    html<Message>().OnBlur(toMessage({ _tag: 'BlurTab' })),
    html<Message>().OnKeyDownPreventDefault(key => {
      const target = tabsKeyboardTarget(input, item.value, key)
      return target === undefined
        ? Option.none()
        : Option.some(toMessage({ _tag: 'KeyDown', value: item.value, key }))
    }),
  ]
}

export const tabs = <Message>(input: TabsProps<Message>): Html => {
  const h = html<Message>()
  const rootId = input.id ?? defaultTabsId
  const selectedValue = tabsSelectedValue(input.items, input.model)
  const orientation = input.orientation ?? 'horizontal'

  return h.div(
    [
      ...basecoatAttrs<Message>(input, tabsRoot),
      h.Id(rootId),
    ],
    [
      h.nav(
        [
          ...(input.tablistAttrs ?? []),
          h.Role('tablist'),
          h.AriaOrientation(orientation),
          ...(input.ariaLabel === undefined
            ? []
            : [h.AriaLabel(input.ariaLabel)]),
          ...dataAttr<Message>(
            'variant',
            input.variant === 'line' ? 'line' : undefined,
          ),
        ],
        input.items.map((item, index) => {
          const selected = item.value === selectedValue && !tabsItemDisabled(item)

          return h.button(
            [
              ...(item.tabAttrs ?? []),
              ...eventAttrs<Message>(input, item),
              h.Type('button'),
              h.Role('tab'),
              h.Id(tabIdFor(rootId, item, index)),
              h.AriaControls(panelIdFor(rootId, item, index)),
              h.AriaSelected(selected),
              h.Tabindex(selected ? 0 : -1),
              ...(item.disabled === true ? [h.Disabled(true)] : []),
              ...(item.ariaDisabled === true ? [h.AriaDisabled(true)] : []),
            ],
            item.tab,
          )
        }),
      ),
      ...input.items
        .filter(item => item.panel !== undefined)
        .map((item, panelIndex) => {
          const itemIndex = input.items.indexOf(item)
          const selected = item.value === selectedValue && !tabsItemDisabled(item)

          return h.div(
            [
              ...(item.panelAttrs ?? []),
              h.Role('tabpanel'),
              h.Id(
                panelIdFor(
                  rootId,
                  item,
                  itemIndex === -1 ? panelIndex : itemIndex,
                ),
              ),
              h.AriaLabelledBy(
                tabIdFor(
                  rootId,
                  item,
                  itemIndex === -1 ? panelIndex : itemIndex,
                ),
              ),
              h.Tabindex(-1),
              h.AriaSelected(selected),
              ...(selected ? [] : [h.Hidden(true)]),
            ],
            item.panel ?? [],
          )
        }),
    ],
  )
}
