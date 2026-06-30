import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  combobox,
  comboboxCanonicalValue,
  comboboxClear,
  comboboxHiddenInput,
  comboboxInit,
  comboboxInput,
  comboboxListbox,
  comboboxOption,
  comboboxPopover,
  comboboxSelectedDetail,
  comboboxSerializedValue,
  comboboxTrigger,
  comboboxUpdate,
  comboboxView,
  type ComboboxMessage,
} from './combobox'
import { renderHtml } from './test-helpers'

const message = (input: ComboboxMessage): ComboboxMessage => input

const options = [
  { value: 'alpha', label: 'Alpha', keywords: ['first'] },
  { value: 'bravo', label: 'Bravo' },
  { value: 'charlie', label: 'Charlie', disabled: true },
] as const

describe('basecoat combobox component', () => {
  test('renders Basecoat combobox markup with input, trigger, clear, popover, and listbox', () => {
    const rendered = renderHtml(
      combobox({
        autoHighlight: true,
        closeOnSelect: true,
        children: [
          comboboxHiddenInput({ name: 'region', value: 'alpha' }),
          comboboxInput({
            id: 'region-input',
            value: 'Alpha',
            placeholder: 'Select region',
            expanded: true,
            controlsId: 'region-listbox',
            activeDescendantId: 'region-alpha',
          }),
          comboboxTrigger({
            expanded: true,
            controlsId: 'region-listbox',
          }),
          comboboxClear({ hidden: false }),
          comboboxPopover({
            id: 'region-popover',
            hidden: false,
            children: [
              comboboxListbox({
                id: 'region-listbox',
                emptyLabel: 'No regions',
                children: [
                  comboboxOption({
                    id: 'region-alpha',
                    value: 'alpha',
                    label: 'Alpha',
                    selected: true,
                    active: true,
                    children: ['Alpha'],
                  }),
                ],
              }),
            ],
          }),
        ],
      }),
    )

    expect(rendered).toContain('class="combobox"')
    expect(rendered).toContain('data-close-on-select="true"')
    expect(rendered).toContain('data-auto-highlight="true"')
    expect(rendered).toContain('type="hidden"')
    expect(rendered).toContain('name="region"')
    expect(rendered).toContain('role="combobox"')
    expect(rendered).toContain('aria-expanded="true"')
    expect(rendered).toContain('aria-controls="region-listbox"')
    expect(rendered).toContain('aria-activedescendant="region-alpha"')
    expect(rendered).toContain('aria-haspopup="listbox"')
    expect(rendered).toContain('data-clear=""')
    expect(rendered).toContain('data-popover=""')
    expect(rendered).toContain('aria-hidden="false"')
    expect(rendered).toContain('role="listbox"')
    expect(rendered).toContain('data-empty="No regions"')
    expect(rendered).toContain('role="option"')
    expect(rendered).toContain('data-value="alpha"')
    expect(rendered).toContain('data-label="Alpha"')
    expect(rendered).toContain('aria-selected="true"')
    expect(rendered).toContain('class="active"')
  })

  test('initializes selected value, visible options, and serialized hidden value', () => {
    const model = comboboxInit({
      options,
      selected: 'alpha',
      autoHighlight: true,
    })

    expect(model.inputValue).toBe('Alpha')
    expect(model.selected).toEqual([{ value: 'alpha', label: 'Alpha' }])
    expect(model.visibleValues).toEqual(['alpha', 'bravo'])
    expect(model.activeValue).toBe('alpha')
    expect(comboboxSerializedValue(model)).toBe('alpha')
    expect(comboboxCanonicalValue(model)).toBe('alpha')
    expect(comboboxSelectedDetail(model)).toEqual({
      value: 'alpha',
      label: 'Alpha',
    })
  })

  test('filters on input and clears single selection while preserving disabled options as hidden', () => {
    const model = comboboxInit({
      options,
      selected: 'alpha',
      autoHighlight: true,
    })
    const filtered = comboboxUpdate(model, {
      _tag: 'ComboboxInputChanged',
      value: 'bra',
    })

    expect(filtered.open).toBe(true)
    expect(filtered.selected).toEqual([])
    expect(filtered.visibleValues).toEqual(['bravo'])
    expect(filtered.activeValue).toBe('bravo')

    const cleared = comboboxUpdate(filtered, { _tag: 'ComboboxClearClicked' })
    expect(cleared.inputValue).toBe('')
    expect(cleared.selected).toEqual([])
    expect(cleared.visibleValues).toEqual(['alpha', 'bravo'])
  })

  test('supports keyboard open, active option movement, selection, and escape close', () => {
    const model = comboboxInit({ options })
    const opened = comboboxUpdate(model, {
      _tag: 'ComboboxKeyDown',
      key: 'ArrowDown',
    })
    expect(opened.open).toBe(true)
    expect(opened.activeValue).toBe('alpha')

    const next = comboboxUpdate(opened, {
      _tag: 'ComboboxKeyDown',
      key: 'ArrowDown',
    })
    expect(next.activeValue).toBe('bravo')

    const selected = comboboxUpdate(next, {
      _tag: 'ComboboxKeyDown',
      key: 'Enter',
    })
    expect(selected.open).toBe(false)
    expect(selected.selected).toEqual([{ value: 'bravo', label: 'Bravo' }])
    expect(selected.inputValue).toBe('Bravo')

    const closed = comboboxUpdate({ ...selected, open: true }, {
      _tag: 'ComboboxKeyDown',
      key: 'Escape',
    })
    expect(closed.open).toBe(false)
    expect(closed.activeValue).toBeNull()
  })

  test('supports multiple selection chips, toggles, backspace removal, and JSON serialization', () => {
    const model = comboboxInit({
      options,
      multiple: true,
      selected: ['alpha', 'bravo'],
      closeOnSelect: false,
    })

    expect(model.inputValue).toBe('')
    expect(comboboxSerializedValue(model)).toBe('["alpha","bravo"]')
    expect(comboboxCanonicalValue(model)).toEqual(['alpha', 'bravo'])

    const toggled = comboboxUpdate(model, {
      _tag: 'ComboboxOptionToggled',
      value: 'alpha',
    })
    expect(toggled.selected).toEqual([{ value: 'bravo', label: 'Bravo' }])

    const removed = comboboxUpdate(toggled, {
      _tag: 'ComboboxKeyDown',
      key: 'Backspace',
    })
    expect(removed.selected).toEqual([])
  })

  test('renders comboboxView with chips, hidden selected options, and object formatted value', () => {
    const model = comboboxInit({
      options,
      multiple: true,
      selected: ['alpha'],
      open: true,
      format: 'object',
      autoHighlight: true,
    })

    const rendered = renderHtml(
      comboboxView({
        id: 'letters',
        name: 'letters',
        model,
        toMessage: message,
        clearable: true,
        trigger: true,
        placeholder: 'Choose letters',
        emptyLabel: 'No letters',
      }),
    )

    expect(rendered).toContain('data-format="object"')
    expect(rendered).toContain('value="[{\"value\":\"alpha\",\"label\":\"Alpha\"}]"')
    expect(rendered).toContain('class="combobox-chips"')
    expect(rendered).toContain('class="combobox-chip"')
    expect(rendered).toContain('aria-label="Remove Alpha"')
    expect(rendered).toContain('aria-multiselectable="true"')
    expect(rendered).toContain('id="letters-option-alpha"')
    expect(rendered).toContain('id="letters-option-charlie"')
    expect(rendered).toContain('aria-disabled="true"')
    expect(rendered).toContain('aria-hidden="true"')
    expect(rendered).toContain('data-value="" data-placeholder="Choose letters">Alpha</span>')
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.combobox).toBe(combobox)
    expect(Basecoat.comboboxInput).toBe(comboboxInput)
    expect(Basecoat.comboboxTrigger).toBe(comboboxTrigger)
    expect(Basecoat.comboboxClear).toBe(comboboxClear)
    expect(Basecoat.comboboxPopover).toBe(comboboxPopover)
    expect(Basecoat.comboboxListbox).toBe(comboboxListbox)
    expect(Basecoat.comboboxOption).toBe(comboboxOption)
    expect(Basecoat.comboboxView).toBe(comboboxView)
    expect(Basecoat.comboboxInit).toBe(comboboxInit)
    expect(Basecoat.comboboxUpdate).toBe(comboboxUpdate)
  })
})
