import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  drawer,
  drawerBody,
  drawerClose,
  drawerCompletedClose,
  drawerDescription,
  drawerFocusedItem,
  drawerFooter,
  drawerHeader,
  drawerItem,
  drawerKeyMessage,
  drawerPanel,
  drawerPressedBackdropPointer,
  drawerPressedKey,
  drawerReleasedBackdropPointer,
  drawerRequestedClose,
  drawerRequestedOpen,
  drawerSelectedItem,
  drawerTitle,
  drawerTrigger,
  initDrawer,
  updateDrawer,
} from './drawer'
import { renderHtml } from './test-helpers'

type Message = Readonly<{
  type: 'drawer'
  message: ReturnType<typeof drawerRequestedOpen>
}>

const wrap = (message: ReturnType<typeof drawerRequestedOpen>): Message => ({
  type: 'drawer',
  message,
})

describe('basecoat drawer component', () => {
  test('updates open, closing, and completed close states', () => {
    const opened = updateDrawer(initDrawer(), drawerRequestedOpen())
    expect(opened.open).toBe(true)
    expect(opened.closing).toBe(false)

    const closing = updateDrawer(opened, drawerRequestedClose())
    expect(closing.open).toBe(true)
    expect(closing.closing).toBe(true)

    const closed = updateDrawer(closing, drawerCompletedClose())
    expect(closed.open).toBe(false)
    expect(closed.closing).toBe(false)
  })

  test('models backdrop pointer close without imperative DOM state', () => {
    const open = initDrawer({ open: true })
    const pressed = updateDrawer(open, drawerPressedBackdropPointer())
    expect(pressed.pointerStartedOnBackdrop).toBe(true)

    const closing = updateDrawer(pressed, drawerReleasedBackdropPointer())
    expect(closing.closing).toBe(true)
  })

  test('models item focus, selection, and keyboard navigation', () => {
    const model = initDrawer({ open: true })
    const focused = updateDrawer(model, drawerFocusedItem(1), 3)
    expect(focused.focusedIndex).toBe(1)

    const next = updateDrawer(focused, drawerPressedKey('ArrowDown'), 3)
    expect(next.focusedIndex).toBe(2)

    const wrapped = updateDrawer(next, drawerPressedKey('ArrowDown'), 3)
    expect(wrapped.focusedIndex).toBe(0)

    const last = updateDrawer(wrapped, drawerPressedKey('End'), 3)
    expect(last.focusedIndex).toBe(2)

    const selected = updateDrawer(last, drawerSelectedItem(1, 'billing', true), 3)
    expect(selected.selectedValue).toBe('billing')
    expect(selected.focusedIndex).toBe(1)
    expect(selected.closing).toBe(true)
  })

  test('maps supported keyboard event keys into drawer messages', () => {
    expect(drawerKeyMessage('Escape')).toEqual(drawerPressedKey('Escape'))
    expect(drawerKeyMessage('ArrowLeft')).toEqual(drawerPressedKey('ArrowLeft'))
    expect(drawerKeyMessage('x')).toBeNull()
  })

  test('renders Basecoat dialog drawer markup with side and closing state', () => {
    const model = updateDrawer(initDrawer({ open: true }), drawerRequestedClose())
    const rendered = renderHtml(
      drawer({
        model,
        side: 'right',
        id: 'settings-drawer',
        labelledBy: 'settings-title',
        describedBy: 'settings-description',
        onMessage: wrap,
        children: [
          drawerPanel({
            model,
            labelledBy: 'settings-title',
            onMessage: wrap,
            children: [
              drawerHeader({
                children: [
                  drawerTitle({ id: 'settings-title', children: ['Settings'] }),
                  drawerDescription({
                    id: 'settings-description',
                    children: ['Manage workspace preferences.'],
                  }),
                ],
              }),
              drawerBody({ children: ['Drawer body'] }),
              drawerFooter({
                children: [
                  drawerClose({
                    onMessage: wrap,
                    ariaLabel: 'Close settings',
                    children: ['Close'],
                  }),
                ],
              }),
            ],
          }),
        ],
      }),
    )

    expect(rendered).toContain('<dialog')
    expect(rendered).toContain('class="drawer"')
    expect(rendered).toContain('open=""')
    expect(rendered).toContain('data-side="right"')
    expect(rendered).toContain('data-closing="true"')
    expect(rendered).toContain('id="settings-drawer"')
    expect(rendered).toContain('aria-labelledby="settings-title"')
    expect(rendered).toContain('aria-describedby="settings-description"')
    expect(rendered).toContain('data-slot="drawer-content"')
    expect(rendered).toContain('<header')
    expect(rendered).toContain('<section')
    expect(rendered).toContain('<footer')
    expect(rendered).toContain('data-slot="drawer-close"')
  })

  test('renders trigger and selectable drawer item controls', () => {
    const model = initDrawer({
      open: true,
      focusedIndex: 1,
      selectedValue: 'logs',
    })
    const rendered = renderHtml(
      drawerPanel({
        children: [
          drawerTrigger({
            onMessage: wrap,
            ariaLabel: 'Open drawer',
            children: ['Open'],
          }),
          drawerItem({
            model,
            index: 1,
            value: 'logs',
            onMessage: wrap,
            closeOnSelect: true,
            children: ['Logs'],
          }),
        ],
      }),
    )

    expect(rendered).toContain('aria-label="Open drawer"')
    expect(rendered).toContain('data-slot="drawer-item"')
    expect(rendered).toContain('data-value="logs"')
    expect(rendered).toContain('aria-selected="true"')
    expect(rendered).toContain('data-selected="true"')
    expect(rendered).toContain('data-focused="true"')
    expect(rendered).toContain('tabIndex="0"')
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.drawer).toBe(drawer)
    expect(Basecoat.drawerPanel).toBe(drawerPanel)
    expect(Basecoat.drawerHeader).toBe(drawerHeader)
    expect(Basecoat.drawerBody).toBe(drawerBody)
    expect(Basecoat.drawerFooter).toBe(drawerFooter)
    expect(Basecoat.drawerTitle).toBe(drawerTitle)
    expect(Basecoat.drawerDescription).toBe(drawerDescription)
    expect(Basecoat.drawerTrigger).toBe(drawerTrigger)
    expect(Basecoat.drawerClose).toBe(drawerClose)
    expect(Basecoat.drawerItem).toBe(drawerItem)
    expect(Basecoat.initDrawer).toBe(initDrawer)
    expect(Basecoat.updateDrawer).toBe(updateDrawer)
  })
})
