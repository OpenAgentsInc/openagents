import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  dropdownMenu,
  dropdownMenuCheckboxItem,
  dropdownMenuGroup,
  dropdownMenuIndicator,
  dropdownMenuInit,
  dropdownMenuItem,
  dropdownMenuLabel,
  dropdownMenuMenu,
  dropdownMenuPopover,
  dropdownMenuRadioItem,
  dropdownMenuSeparator,
  dropdownMenuShortcut,
  dropdownMenuTrigger,
  dropdownMenuUpdate,
  dropdownMenuView,
  type DropdownMenuMessage,
} from './dropdown-menu'
import { renderHtml } from './test-helpers'

const message = (input: DropdownMenuMessage): DropdownMenuMessage => input

describe('basecoat dropdown-menu component', () => {
  test('renders Basecoat dropdown markup, roles, and state attrs', () => {
    const rendered = renderHtml(
      dropdownMenu({
        id: 'actions',
        initialized: true,
        children: [
          dropdownMenuTrigger({
            id: 'actions-trigger',
            controlsId: 'actions-menu',
            open: true,
            activeDescendantId: 'actions-menu-archive',
            children: ['Actions'],
          }),
          dropdownMenuPopover({
            id: 'actions-popover',
            open: true,
            children: [
              dropdownMenuMenu({
                id: 'actions-menu',
                labelledBy: 'actions-trigger',
                children: [
                  dropdownMenuItem({
                    id: 'actions-menu-archive',
                    active: true,
                    shortcut: ['A'],
                    children: ['Archive'],
                  }),
                  dropdownMenuSeparator({}),
                  dropdownMenuItem({
                    type: 'checkbox',
                    checked: true,
                    indicator: ['check'],
                    children: ['Pinned'],
                  }),
                ],
              }),
            ],
          }),
        ],
      }),
    )

    expect(rendered).toContain('class="dropdown-menu"')
    expect(rendered).toContain('data-dropdown-menu-initialized="true"')
    expect(rendered).toContain('aria-haspopup="menu"')
    expect(rendered).toContain('aria-expanded="true"')
    expect(rendered).toContain('aria-controls="actions-menu"')
    expect(rendered).toContain('aria-activedescendant="actions-menu-archive"')
    expect(rendered).toContain('data-popover=""')
    expect(rendered).toContain('aria-hidden="false"')
    expect(rendered).toContain('role="menu"')
    expect(rendered).toContain('aria-labelledby="actions-trigger"')
    expect(rendered).toContain('role="menuitem"')
    expect(rendered).toContain('class="active"')
    expect(rendered).toContain('data-shortcut=""')
    expect(rendered).toContain('role="separator"')
    expect(rendered).toContain('role="menuitemcheckbox"')
    expect(rendered).toContain('aria-checked="true"')
    expect(rendered).toContain('data-indicator=""')
  })

  test('renders disabled links, radio items, groups, and labels', () => {
    const rendered = renderHtml(
      dropdownMenuMenu({
        children: [
          dropdownMenuGroup({
            labelledBy: 'view-label',
            children: [
              dropdownMenuLabel({
                id: 'view-label',
                children: ['View'],
              }),
              dropdownMenuItem({
                href: '/settings',
                disabled: true,
                children: ['Settings'],
              }),
              dropdownMenuItem({
                type: 'radio',
                checked: false,
                indicator: ['selected'],
                children: ['Compact'],
              }),
            ],
          }),
        ],
      }),
    )

    expect(rendered).toContain('role="group"')
    expect(rendered).toContain('aria-labelledby="view-label"')
    expect(rendered).toContain('role="heading"')
    expect(rendered).toContain('<a')
    expect(rendered).toContain('href="/settings"')
    expect(rendered).toContain('aria-disabled="true"')
    expect(rendered).toContain('role="menuitemradio"')
    expect(rendered).toContain('aria-checked="false"')
  })

  test('initializes open state with first or last active item and normalized selection', () => {
    const model = dropdownMenuInit({
      open: true,
      initialSelection: 'last',
      items: [
        { value: 'archive' },
        { value: 'disabled', disabled: true },
        { value: 'pin', type: 'checkbox' },
        { value: 'compact', type: 'radio', radioGroup: 'density' },
      ],
      checkedValues: ['pin', 'missing'],
      radioValues: {
        density: 'compact',
        missing: 'ghost',
      },
    })

    expect(model.open).toBe(true)
    expect(model.activeValue).toBe('compact')
    expect(model.checkedValues).toEqual(['pin'])
    expect(model.radioValues).toEqual({ density: 'compact' })
  })

  test('ports keyboard behavior from Basecoat dropdown-menu JS', () => {
    const model = dropdownMenuInit({
      items: [
        { value: 'archive' },
        { value: 'disabled', disabled: true },
        { value: 'pin', type: 'checkbox' },
      ],
    })

    const opened = dropdownMenuUpdate(model, {
      _tag: 'DropdownMenuKeyDown',
      key: 'ArrowDown',
    })
    expect(opened.open).toBe(true)
    expect(opened.activeValue).toBe('archive')

    const next = dropdownMenuUpdate(opened, {
      _tag: 'DropdownMenuKeyDown',
      key: 'ArrowDown',
    })
    expect(next.activeValue).toBe('pin')

    const home = dropdownMenuUpdate(next, {
      _tag: 'DropdownMenuKeyDown',
      key: 'Home',
    })
    expect(home.activeValue).toBe('archive')

    const closed = dropdownMenuUpdate(home, {
      _tag: 'DropdownMenuKeyDown',
      key: 'Escape',
    })
    expect(closed.open).toBe(false)
    expect(closed.activeValue).toBe(null)
  })

  test('selects normal items, toggles checkboxes, and sets radio groups', () => {
    const model = dropdownMenuInit({
      open: true,
      items: [
        { value: 'archive' },
        { value: 'pin', type: 'checkbox' },
        { value: 'comfortable', type: 'radio', radioGroup: 'density' },
        { value: 'compact', type: 'radio', radioGroup: 'density' },
      ],
      checkedValues: ['pin'],
      radioValues: {
        density: 'comfortable',
      },
    })

    const unchecked = dropdownMenuUpdate(model, {
      _tag: 'DropdownMenuItemActivated',
      value: 'pin',
    })
    expect(unchecked.open).toBe(false)
    expect(unchecked.checkedValues).toEqual([])
    expect(unchecked.selectedValue).toBe('pin')

    const radio = dropdownMenuUpdate(model, {
      _tag: 'DropdownMenuItemActivated',
      value: 'compact',
    })
    expect(radio.radioValues).toEqual({ density: 'compact' })
    expect(radio.selectedValue).toBe('compact')

    const normal = dropdownMenuUpdate(model, {
      _tag: 'DropdownMenuItemActivated',
      value: 'archive',
    })
    expect(normal.selectedValue).toBe('archive')
    expect(normal.open).toBe(false)
  })

  test('dropdownMenuView wires model state into rendered attrs', () => {
    const model = dropdownMenuInit({
      open: true,
      initialSelection: 'first',
      items: [
        { value: 'archive' },
        { value: 'pin', type: 'checkbox' },
        { value: 'compact', type: 'radio', radioGroup: 'density' },
      ],
      checkedValues: ['pin'],
      radioValues: { density: 'compact' },
    })

    const rendered = renderHtml(
      dropdownMenuView({
        id: 'thread-actions',
        model,
        toMessage: message,
        trigger: ['Actions'],
        items: [
          {
            value: 'archive',
            children: ['Archive'],
            shortcut: ['A'],
          },
          { type: 'separator' },
          {
            value: 'pin',
            itemType: 'checkbox',
            indicator: ['check'],
            children: ['Pinned'],
          },
          {
            type: 'group',
            label: ['Density'],
            children: [
              {
                value: 'compact',
                itemType: 'radio',
                radioGroup: 'density',
                indicator: ['selected'],
                children: ['Compact'],
              },
            ],
          },
        ],
      }),
    )

    expect(rendered).toContain('id="thread-actions"')
    expect(rendered).toContain('aria-expanded="true"')
    expect(rendered).toContain('aria-activedescendant="thread-actions-menu-archive"')
    expect(rendered).toContain('id="thread-actions-menu-archive"')
    expect(rendered).toContain('class="active"')
    expect(rendered).toContain('role="menuitemcheckbox"')
    expect(rendered).toContain('aria-checked="true"')
    expect(rendered).toContain('role="menuitemradio"')
    expect(rendered).toContain('role="group"')
    expect(rendered).toContain('role="heading"')
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.dropdownMenu).toBe(dropdownMenu)
    expect(Basecoat.dropdownMenuTrigger).toBe(dropdownMenuTrigger)
    expect(Basecoat.dropdownMenuPopover).toBe(dropdownMenuPopover)
    expect(Basecoat.dropdownMenuMenu).toBe(dropdownMenuMenu)
    expect(Basecoat.dropdownMenuItem).toBe(dropdownMenuItem)
    expect(Basecoat.dropdownMenuCheckboxItem).toBe(dropdownMenuCheckboxItem)
    expect(Basecoat.dropdownMenuRadioItem).toBe(dropdownMenuRadioItem)
    expect(Basecoat.dropdownMenuGroup).toBe(dropdownMenuGroup)
    expect(Basecoat.dropdownMenuLabel).toBe(dropdownMenuLabel)
    expect(Basecoat.dropdownMenuSeparator).toBe(dropdownMenuSeparator)
    expect(Basecoat.dropdownMenuShortcut).toBe(dropdownMenuShortcut)
    expect(Basecoat.dropdownMenuIndicator).toBe(dropdownMenuIndicator)
    expect(Basecoat.dropdownMenuView).toBe(dropdownMenuView)
    expect(Basecoat.dropdownMenuInit).toBe(dropdownMenuInit)
    expect(Basecoat.dropdownMenuUpdate).toBe(dropdownMenuUpdate)
  })
})
