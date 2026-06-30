import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  selectInit,
  selectSerializedValue,
  selectUpdate,
  selectValue,
  selectView,
  type SelectMessage,
} from './select'
import { renderHtml } from './test-helpers'

const message = (input: SelectMessage): SelectMessage => input

const options = [
  { value: 'alpha', label: 'Alpha' },
  { value: 'bravo', label: 'Bravo' },
  { value: 'charlie', label: 'Charlie', disabled: true },
  { value: 'delta', label: 'Delta' },
]

describe('basecoat select component', () => {
  test('renders Basecoat select markup with trigger, popover, listbox, options, and hidden input', () => {
    const model = selectInit({
      options,
      value: 'bravo',
      placeholder: 'Pick one',
    })

    const rendered = renderHtml(
      selectView({
        model,
        name: 'agent',
        listboxId: 'agent-listbox',
        emptyText: 'No agents',
        toMessage: message,
      }),
    )

    expect(rendered).toContain('class="select"')
    expect(rendered).toContain('type="hidden"')
    expect(rendered).toContain('name="agent"')
    expect(rendered).toContain('value="bravo"')
    expect(rendered).toContain('role="combobox"')
    expect(rendered).toContain('aria-haspopup="listbox"')
    expect(rendered).toContain('aria-expanded="false"')
    expect(rendered).toContain('aria-controls="agent-listbox"')
    expect(rendered).toContain('data-popover=""')
    expect(rendered).toContain('aria-hidden="true"')
    expect(rendered).toContain('role="listbox"')
    expect(rendered).toContain('data-empty="No agents"')
    expect(rendered).toContain('data-value="bravo"')
    expect(rendered).toContain('data-label="Bravo"')
    expect(rendered).toContain('aria-selected="true"')
    expect(rendered).toContain('aria-disabled="true"')
    expect(rendered).toContain('<svg')
  })

  test('renders placeholder styling when no value is selected', () => {
    const model = selectInit({
      options,
      placeholder: 'Pick one',
    })

    const rendered = renderHtml(
      selectView({
        model,
        listboxId: 'agent-listbox',
        toMessage: message,
      }),
    )

    expect(rendered).toContain('data-placeholder="Pick one"')
    expect(rendered).toContain('class="text-muted-foreground"')
    expect(rendered).toContain('Pick one')
  })

  test('initializes single and multiple values from enabled options only', () => {
    const single = selectInit({
      options,
      value: ['alpha', 'bravo', 'charlie'],
    })
    expect(single.selectedValues).toEqual(['alpha'])
    expect(single.activeValue).toBe('alpha')

    const multiple = selectInit({
      options,
      multiple: true,
      value: ['alpha', 'charlie', 'delta', 'alpha'],
    })
    expect(multiple.selectedValues).toEqual(['alpha', 'delta'])
  })

  test('opens, closes, and toggles active focus state', () => {
    const model = selectInit({
      options,
      value: 'bravo',
    })

    const opened = selectUpdate(model, { _tag: 'SelectOpened' })
    expect(opened.open).toBe(true)
    expect(opened.activeValue).toBe('bravo')

    const closed = selectUpdate(opened, { _tag: 'SelectClosed' })
    expect(closed.open).toBe(false)
    expect(closed.activeValue).toBeNull()

    expect(selectUpdate(closed, { _tag: 'SelectToggled' }).open).toBe(true)
  })

  test('selects one value and ignores disabled values', () => {
    const model = selectInit({
      options,
      value: 'alpha',
      open: true,
    })

    const selected = selectUpdate(model, {
      _tag: 'SelectSelected',
      value: 'delta',
    })
    expect(selected.selectedValues).toEqual(['delta'])
    expect(selected.open).toBe(false)

    expect(selectUpdate(selected, {
      _tag: 'SelectSelected',
      value: 'charlie',
    })).toEqual(selected)
  })

  test('toggles multiple values and honors closeOnSelect', () => {
    const model = selectInit({
      options,
      multiple: true,
      closeOnSelect: true,
      value: ['alpha'],
      open: true,
    })

    const added = selectUpdate(model, {
      _tag: 'SelectSelected',
      value: 'delta',
    })
    expect(added.selectedValues).toEqual(['alpha', 'delta'])
    expect(added.open).toBe(false)

    const removed = selectUpdate({ ...added, open: true }, {
      _tag: 'SelectSelected',
      value: 'alpha',
    })
    expect(removed.selectedValues).toEqual(['delta'])
  })

  test('moves active option with keyboard navigation and selects with Enter', () => {
    const model = selectInit({
      options,
      open: true,
      activeValue: 'alpha',
    })

    const down = selectUpdate(model, { _tag: 'SelectKeyDown', key: 'ArrowDown' })
    expect(down.activeValue).toBe('bravo')

    const end = selectUpdate(down, { _tag: 'SelectKeyDown', key: 'End' })
    expect(end.activeValue).toBe('delta')

    const up = selectUpdate(end, { _tag: 'SelectKeyDown', key: 'ArrowUp' })
    expect(up.activeValue).toBe('bravo')

    const selected = selectUpdate(up, { _tag: 'SelectKeyDown', key: 'Enter' })
    expect(selected.selectedValues).toEqual(['bravo'])
    expect(selected.open).toBe(false)
  })

  test('opens from closed state with navigation keys and closes with Escape', () => {
    const model = selectInit({ options })

    const opened = selectUpdate(model, {
      _tag: 'SelectKeyDown',
      key: 'ArrowDown',
    })
    expect(opened.open).toBe(true)

    const closed = selectUpdate(opened, {
      _tag: 'SelectKeyDown',
      key: 'Escape',
    })
    expect(closed.open).toBe(false)
  })

  test('serializes selected values like Basecoat value and object formats', () => {
    const single = selectInit({
      options,
      value: 'alpha',
    })
    expect(selectValue(single)).toBe('alpha')
    expect(selectSerializedValue(single)).toBe('alpha')

    const multipleObject = selectInit({
      options,
      multiple: true,
      format: 'object',
      value: ['alpha', 'delta'],
    })
    expect(selectValue(multipleObject)).toEqual(['alpha', 'delta'])
    expect(selectSerializedValue(multipleObject)).toBe(
      JSON.stringify([
        { value: 'alpha', label: 'Alpha' },
        { value: 'delta', label: 'Delta' },
      ]),
    )
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.selectInit).toBe(selectInit)
    expect(Basecoat.selectUpdate).toBe(selectUpdate)
    expect(Basecoat.selectView).toBe(selectView)
  })
})
