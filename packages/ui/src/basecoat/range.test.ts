import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  rangeInit,
  rangePercent,
  rangeUpdate,
  rangeValue,
  rangeView,
  type RangeMessage,
} from './range'
import { renderHtml } from './test-helpers'

const message = (input: RangeMessage): RangeMessage => input

describe('basecoat range component', () => {
  test('renders Basecoat range markup with value, bounds, step, and CSS percent', () => {
    const model = rangeInit({
      min: 10,
      max: 30,
      step: 5,
      value: 20,
    })

    const rendered = renderHtml(
      rangeView({
        model,
        id: 'temperature',
        name: 'temperature',
        ariaLabel: 'Temperature',
        valueText: '20 degrees',
        toMessage: message,
      }),
    )

    expect(rendered).toContain('class="input"')
    expect(rendered).toContain('type="range"')
    expect(rendered).toContain('id="temperature"')
    expect(rendered).toContain('name="temperature"')
    expect(rendered).toContain('value="20"')
    expect(rendered).toContain('min="10"')
    expect(rendered).toContain('max="30"')
    expect(rendered).toContain('step="5"')
    expect(rendered).toContain('data-range-initialized="true"')
    expect(rendered).toContain('aria-label="Temperature"')
    expect(rendered).toContain('aria-valuemin="10"')
    expect(rendered).toContain('aria-valuemax="30"')
    expect(rendered).toContain('aria-valuenow="20"')
    expect(rendered).toContain('aria-valuetext="20 degrees"')
    expect(rendered).toContain('style="--slider-value: 50%"')
  })

  test('initializes defaults and normalizes value into range bounds and step', () => {
    expect(rangeInit()).toMatchObject({
      min: 0,
      max: 100,
      step: 1,
      value: 0,
      focused: false,
      dragging: false,
    })

    const model = rangeInit({
      min: 0,
      max: 10,
      step: 2,
      value: '7',
    })

    expect(model.value).toBe(8)
    expect(rangeValue(model)).toBe(8)
    expect(rangePercent(model)).toBe(80)
  })

  test('keeps decimal steps precise and supports step any', () => {
    const stepped = rangeInit({
      min: 0,
      max: 1,
      step: 0.1,
      value: 0.26,
    })
    expect(stepped.value).toBe(0.3)

    const any = rangeUpdate(
      rangeInit({
        min: 0,
        max: 1,
        step: 'any',
        value: 0.2,
      }),
      { _tag: 'RangeValueChanged', value: '0.26' },
    )
    expect(any.value).toBe(0.26)
  })

  test('updates from input changes and ignores invalid values', () => {
    const model = rangeInit({
      min: 0,
      max: 100,
      value: 20,
    })

    const changed = rangeUpdate(model, {
      _tag: 'RangeValueChanged',
      value: '140',
    })
    expect(changed.value).toBe(100)

    expect(rangeUpdate(changed, {
      _tag: 'RangeValueChanged',
      value: 'not-a-number',
    })).toEqual(changed)
  })

  test('tracks focus and drag state', () => {
    const model = rangeInit({ value: 20 })

    const focused = rangeUpdate(model, { _tag: 'RangeFocused' })
    expect(focused.focused).toBe(true)

    const dragging = rangeUpdate(focused, { _tag: 'RangeDragStarted' })
    expect(dragging.dragging).toBe(true)
    expect(dragging.focused).toBe(true)

    const ended = rangeUpdate(dragging, { _tag: 'RangeDragEnded' })
    expect(ended.dragging).toBe(false)

    const blurred = rangeUpdate(dragging, { _tag: 'RangeBlurred' })
    expect(blurred.focused).toBe(false)
    expect(blurred.dragging).toBe(false)
  })

  test('moves value with keyboard navigation', () => {
    const model = rangeInit({
      min: 0,
      max: 100,
      step: 5,
      value: 50,
    })

    expect(rangeUpdate(model, { _tag: 'RangeKeyDown', key: 'ArrowRight' }).value).toBe(55)
    expect(rangeUpdate(model, { _tag: 'RangeKeyDown', key: 'ArrowLeft' }).value).toBe(45)
    expect(rangeUpdate(model, { _tag: 'RangeKeyDown', key: 'PageUp' }).value).toBe(100)
    expect(rangeUpdate(model, { _tag: 'RangeKeyDown', key: 'PageDown' }).value).toBe(0)
    expect(rangeUpdate(model, { _tag: 'RangeKeyDown', key: 'Home' }).value).toBe(0)
    expect(rangeUpdate(model, { _tag: 'RangeKeyDown', key: 'End' }).value).toBe(100)
  })

  test('renders disabled range without interaction state markers', () => {
    const model = rangeInit({ value: 40 })
    const rendered = renderHtml(
      rangeView({
        model,
        disabled: true,
        toMessage: message,
      }),
    )

    expect(rendered).toContain('disabled')
    expect(rendered).not.toContain('data-focused')
    expect(rendered).not.toContain('data-dragging')
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.rangeInit).toBe(rangeInit)
    expect(Basecoat.rangeUpdate).toBe(rangeUpdate)
    expect(Basecoat.rangeView).toBe(rangeView)
  })
})
