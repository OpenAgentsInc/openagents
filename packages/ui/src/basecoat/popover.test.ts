import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  PopoverClosed,
  PopoverItemFocused,
  PopoverKeyPressed,
  PopoverOpened,
  PopoverOutsidePressed,
  PopoverPeerOpened,
  PopoverToggled,
  popover,
  popoverContent,
  popoverInit,
  popoverItem,
  popoverTrigger,
  popoverUpdate,
} from './popover'
import { renderHtml } from './test-helpers'

describe('basecoat popover component', () => {
  test('renders Basecoat popover markup and open state attributes', () => {
    const model = popoverInit({ id: 'actions', open: true })
    const rendered = renderHtml(
      popover({
        model,
        side: 'inline-end',
        align: 'start',
        trigger: ['More'],
        triggerClassName: 'btn',
        triggerLabel: 'Open actions',
        contentClassName: 'rounded-md border bg-popover',
        contentRole: 'menu',
        toMessage: message => message,
        children: ['Actions'],
      }),
    )

    expect(rendered).toContain('<div')
    expect(rendered).toContain('class="popover"')
    expect(rendered).toContain('id="actions-trigger"')
    expect(rendered).toContain('type="button"')
    expect(rendered).toContain('aria-expanded="true"')
    expect(rendered).toContain('aria-controls="actions-content"')
    expect(rendered).toContain('data-popover-trigger=""')
    expect(rendered).toContain('aria-label="Open actions"')
    expect(rendered).toContain('id="actions-content"')
    expect(rendered).toContain('data-popover=""')
    expect(rendered).toContain('aria-hidden="false"')
    expect(rendered).toContain('role="menu"')
    expect(rendered).toContain('data-side="inline-end"')
    expect(rendered).toContain('data-align="start"')
    expect(rendered).toContain('class="rounded-md border bg-popover"')
  })

  test('renders trigger and content slots independently', () => {
    const model = popoverInit({ id: 'filters' })
    const trigger = renderHtml(
      popoverTrigger({
        model,
        toMessage: message => message,
        children: ['Filters'],
      }),
    )
    const content = renderHtml(
      popoverContent({
        model,
        toMessage: message => message,
        children: ['Filter controls'],
      }),
    )

    expect(trigger).toContain('aria-expanded="false"')
    expect(trigger).toContain('data-popover-trigger=""')
    expect(content).toContain('aria-hidden="true"')
    expect(content).toContain('data-popover=""')
    expect(content).toContain('Filter controls')
  })

  test('updates open, close, outside click, and peer coordination state', () => {
    const closed = popoverInit({ id: 'one' })
    const opened = popoverUpdate(closed, PopoverToggled({ id: 'one' }))

    expect(opened.model.open).toBe(true)
    expect(opened.effect).toEqual({
      _tag: 'opened',
      sourceId: 'one',
      focus: 'content',
    })

    const peerClosed = popoverUpdate(
      opened.model,
      PopoverPeerOpened({ id: 'one', sourceId: 'two' }),
    )

    expect(peerClosed.model.open).toBe(false)
    expect(peerClosed.effect).toEqual({ _tag: 'closed', focus: 'none' })

    const reopened = popoverUpdate(peerClosed.model, PopoverOpened({ id: 'one' }))
    const outsideClosed = popoverUpdate(
      reopened.model,
      PopoverOutsidePressed({ id: 'one' }),
    )

    expect(outsideClosed.model.open).toBe(false)
    expect(outsideClosed.effect).toEqual({ _tag: 'closed', focus: 'none' })

    const ignored = popoverUpdate(
      outsideClosed.model,
      PopoverClosed({ id: 'other' }),
    )

    expect(ignored.model).toBe(outsideClosed.model)
    expect(ignored.effect).toEqual({ _tag: 'none' })
  })

  test('handles keyboard navigation and item selection in the reducer', () => {
    const model = popoverInit({ id: 'menu', itemCount: 3 })
    const opened = popoverUpdate(
      model,
      PopoverKeyPressed({ id: 'menu', key: 'ArrowDown' }),
    )

    expect(opened.model.open).toBe(true)
    expect(opened.model.activeIndex).toBe(0)
    expect(opened.effect).toEqual({
      _tag: 'opened',
      sourceId: 'menu',
      focus: 'item',
    })

    const moved = popoverUpdate(
      opened.model,
      PopoverKeyPressed({ id: 'menu', key: 'End' }),
    )

    expect(moved.model.activeIndex).toBe(2)
    expect(moved.effect).toEqual({ _tag: 'focusedItem', index: 2 })

    const selected = popoverUpdate(
      moved.model,
      PopoverKeyPressed({ id: 'menu', key: 'Enter' }),
    )

    expect(selected.model.selectedIndex).toBe(2)
    expect(selected.model.open).toBe(false)
    expect(selected.effect).toEqual({ _tag: 'selected', index: 2, closed: true })
  })

  test('renders active and selected popover items', () => {
    const model = popoverInit({
      id: 'menu',
      open: true,
      itemCount: 2,
      activeIndex: 1,
      selectedIndex: 1,
    })
    const rendered = renderHtml(
      popoverItem({
        model,
        index: 1,
        toMessage: message => message,
        children: ['Archive'],
      }),
    )

    expect(rendered).toContain('role="menuitem"')
    expect(rendered).toContain('data-popover-item=""')
    expect(rendered).toContain('data-index="1"')
    expect(rendered).toContain('data-active=""')
    expect(rendered).toContain('aria-selected="true"')
    expect(rendered).toContain('tabIndex="0"')

    const focused = popoverUpdate(
      model,
      PopoverItemFocused({ id: 'menu', index: 0 }),
    )

    expect(focused.model.activeIndex).toBe(0)
    expect(focused.effect).toEqual({ _tag: 'focusedItem', index: 0 })
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.popover).toBe(popover)
    expect(Basecoat.popoverTrigger).toBe(popoverTrigger)
    expect(Basecoat.popoverContent).toBe(popoverContent)
    expect(Basecoat.popoverItem).toBe(popoverItem)
    expect(Basecoat.popoverInit).toBe(popoverInit)
    expect(Basecoat.popoverUpdate).toBe(popoverUpdate)
  })
})
