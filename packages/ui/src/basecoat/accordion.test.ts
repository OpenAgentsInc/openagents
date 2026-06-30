import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  accordion,
  accordionContent,
  accordionInit,
  accordionItem,
  accordionTrigger,
  accordionUpdate,
  accordionView,
  type AccordionMessage,
} from './accordion'
import { renderHtml } from './test-helpers'

const message = (input: AccordionMessage): AccordionMessage => input

describe('basecoat accordion component', () => {
  test('renders Basecoat details and summary markup', () => {
    const rendered = renderHtml(
      accordion({
        children: [
          accordionItem({
            value: 'shipping',
            open: true,
            children: [
              accordionTrigger({
                itemValue: 'shipping',
                controlsId: 'shipping-content',
                open: true,
                tabIndex: 0,
                children: ['Shipping'],
              }),
              accordionContent({
                id: 'shipping-content',
                labelledBy: 'shipping-trigger',
                children: ['Ships in 2 days.'],
              }),
            ],
          }),
        ],
      }),
    )

    expect(rendered).toContain('class="accordion"')
    expect(rendered).toContain('<details')
    expect(rendered).toContain('data-value="shipping"')
    expect(rendered).toContain('open')
    expect(rendered).toContain('<summary')
    expect(rendered).toContain('aria-expanded="true"')
    expect(rendered).toContain('aria-controls="shipping-content"')
    expect(rendered).toContain('tabIndex="0"')
    expect(rendered).toContain('<svg')
    expect(rendered).toContain('id="shipping-content"')
    expect(rendered).toContain('aria-labelledby="shipping-trigger"')
  })

  test('adds multiple and disabled Basecoat state attrs', () => {
    const rendered = renderHtml(
      accordion({
        type: 'multiple',
        children: [
          accordionItem({
            value: 'locked',
            disabled: true,
            children: [
              accordionTrigger({
                itemValue: 'locked',
                disabled: true,
                children: ['Locked'],
              }),
            ],
          }),
        ],
      }),
    )

    expect(rendered).toContain('data-multiple="true"')
    expect(rendered).toContain('aria-disabled="true"')
    expect(rendered).toContain('data-disabled="true"')
  })

  test('initializes single accordions with only one enabled open item', () => {
    const model = accordionInit({
      items: [
        { value: 'first' },
        { value: 'second' },
        { value: 'disabled', disabled: true },
      ],
      defaultOpenValues: ['first', 'second', 'disabled'],
    })

    expect(model.type).toBe('single')
    expect(model.openValues).toEqual(['first'])
    expect(model.focusedValue).toBe('first')
    expect(model.selectedValue).toBe('first')
  })

  test('opens one item at a time in single mode and preserves siblings in multiple mode', () => {
    const single = accordionInit({
      items: [{ value: 'first' }, { value: 'second' }],
      defaultOpenValues: ['first'],
    })

    expect(accordionUpdate(single, {
      _tag: 'AccordionToggled',
      value: 'second',
      open: true,
    }).openValues).toEqual(['second'])

    const multiple = accordionInit({
      type: 'multiple',
      items: [{ value: 'first' }, { value: 'second' }],
      defaultOpenValues: ['first'],
    })

    expect(accordionUpdate(multiple, {
      _tag: 'AccordionToggled',
      value: 'second',
      open: true,
    }).openValues).toEqual(['first', 'second'])
  })

  test('ignores disabled item toggles and selection', () => {
    const model = accordionInit({
      items: [{ value: 'first' }, { value: 'disabled', disabled: true }],
      defaultOpenValues: ['first'],
    })

    expect(accordionUpdate(model, {
      _tag: 'AccordionToggled',
      value: 'disabled',
      open: true,
    })).toEqual(model)
    expect(accordionUpdate(model, {
      _tag: 'AccordionSelected',
      value: 'disabled',
    })).toEqual(model)
  })

  test('moves focus with accordion keyboard navigation', () => {
    const model = accordionInit({
      items: [
        { value: 'first' },
        { value: 'disabled', disabled: true },
        { value: 'second' },
      ],
    })

    const next = accordionUpdate(model, {
      _tag: 'AccordionKeyDown',
      value: 'first',
      key: 'ArrowDown',
    })
    expect(next.focusedValue).toBe('second')

    const home = accordionUpdate(next, {
      _tag: 'AccordionKeyDown',
      value: 'second',
      key: 'Home',
    })
    expect(home.focusedValue).toBe('first')

    const end = accordionUpdate(home, {
      _tag: 'AccordionKeyDown',
      value: 'first',
      key: 'End',
    })
    expect(end.focusedValue).toBe('second')
  })

  test('keyboard Enter and Space toggle the focused item', () => {
    const model = accordionInit({
      items: [{ value: 'first' }, { value: 'second' }],
    })

    const opened = accordionUpdate(model, {
      _tag: 'AccordionKeyDown',
      value: 'second',
      key: 'Enter',
    })
    expect(opened.openValues).toEqual(['second'])
    expect(opened.focusedValue).toBe('second')

    const closed = accordionUpdate(opened, {
      _tag: 'AccordionKeyDown',
      value: 'second',
      key: ' ',
    })
    expect(closed.openValues).toEqual([])
  })

  test('accordionView wires model state into rendered attrs', () => {
    const model = accordionInit({
      type: 'multiple',
      items: [{ value: 'shipping' }, { value: 'returns' }],
      defaultOpenValues: ['returns'],
      focusedValue: 'returns',
    })

    const rendered = renderHtml(
      accordionView({
        model,
        toMessage: message,
        items: [
          {
            value: 'shipping',
            trigger: ['Shipping'],
            content: ['Shipping details'],
          },
          {
            value: 'returns',
            trigger: ['Returns'],
            content: ['Return details'],
          },
        ],
      }),
    )

    expect(rendered).toContain('data-multiple="true"')
    expect(rendered).toContain('data-value="returns" open')
    expect(rendered).toContain('aria-expanded="true"')
    expect(rendered).toContain('tabIndex="0"')
    expect(rendered).toContain('hidden')
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.accordion).toBe(accordion)
    expect(Basecoat.accordionItem).toBe(accordionItem)
    expect(Basecoat.accordionTrigger).toBe(accordionTrigger)
    expect(Basecoat.accordionContent).toBe(accordionContent)
    expect(Basecoat.accordionView).toBe(accordionView)
    expect(Basecoat.accordionInit).toBe(accordionInit)
    expect(Basecoat.accordionUpdate).toBe(accordionUpdate)
  })
})
