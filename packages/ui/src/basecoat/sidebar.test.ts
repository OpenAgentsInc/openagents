import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  SidebarActivatedItem,
  SidebarClosed,
  SidebarClickedOverlay,
  SidebarFocusedItem,
  SidebarPressedKey,
  SidebarToggled,
  SidebarToggledSubmenu,
  initSidebar,
  isMobileSidebar,
  sidebar,
  sidebarItemIds,
  sidebarKeepMobileOpenItemIds,
  sidebarMessageActions,
  sidebarVisibleItemIds,
  updateSidebar,
} from './sidebar'
import { renderHtml } from './test-helpers'

const sampleMenu = [
  {
    type: 'group',
    id: 'getting-started',
    label: ['Getting started'],
    items: [
      {
        id: 'playground',
        label: ['Playground'],
        href: '/playground',
        current: true,
        variant: 'outline',
        size: 'sm',
        keepMobileSidebarOpen: true,
      },
      {
        id: 'models',
        label: ['Models'],
        href: '/models',
      },
      {
        type: 'submenu',
        id: 'settings',
        label: ['Settings'],
        items: [
          {
            id: 'general',
            label: ['General'],
            href: '/settings/general',
          },
          {
            id: 'billing',
            label: ['Billing'],
            href: '/settings/billing',
          },
        ],
      },
    ],
  },
  {
    type: 'separator',
    id: 'main-separator',
  },
  {
    id: 'logout',
    label: ['Log out'],
    disabled: true,
  },
] as const

describe('basecoat sidebar model', () => {
  test('resolves initial desktop and mobile open state from the breakpoint', () => {
    const desktop = initSidebar({
      initialOpen: false,
      initialMobileOpen: true,
      viewportWidth: 1024,
    })
    const mobile = initSidebar({
      initialOpen: false,
      initialMobileOpen: true,
      viewportWidth: 375,
    })

    expect(desktop.open).toBe(false)
    expect(isMobileSidebar(desktop)).toBe(false)
    expect(mobile.open).toBe(true)
    expect(isMobileSidebar(mobile)).toBe(true)
  })

  test('opens, closes, toggles, and closes from overlay messages', () => {
    const start = initSidebar()
    const toggled = updateSidebar(start, SidebarToggled())
    const reopened = updateSidebar(toggled, SidebarToggled())
    const closed = updateSidebar(reopened, SidebarClosed())
    const overlayClosed = updateSidebar(reopened, SidebarClickedOverlay())

    expect(start.open).toBe(true)
    expect(toggled.open).toBe(false)
    expect(reopened.open).toBe(true)
    expect(closed.open).toBe(false)
    expect(overlayClosed.open).toBe(false)
  })

  test('selects activated items and closes mobile unless the item opts out', () => {
    const mobile = initSidebar({
      initialMobileOpen: true,
      viewportWidth: 390,
    })
    const closed = updateSidebar(
      mobile,
      SidebarActivatedItem({ itemId: 'models' }),
    )
    const keptOpen = updateSidebar(
      mobile,
      SidebarActivatedItem({
        itemId: 'playground',
        keepMobileSidebarOpen: true,
      }),
    )

    expect(closed.selectedItemId).toBe('models')
    expect(closed.open).toBe(false)
    expect(closed.focusedItemId).toBe(null)
    expect(keptOpen.selectedItemId).toBe('playground')
    expect(keptOpen.open).toBe(true)
    expect(keptOpen.focusedItemId).toBe('playground')
  })

  test('moves focus and activates the focused item from keyboard messages', () => {
    const itemIds = sidebarItemIds(sampleMenu)
    const focused = updateSidebar(
      initSidebar({ selectedItemId: 'models' }),
      SidebarPressedKey({ key: 'ArrowDown', itemIds }),
    )
    const selected = updateSidebar(
      focused,
      SidebarPressedKey({
        key: 'Enter',
        itemIds,
        keepMobileSidebarOpenItemIds: ['settings'],
      }),
    )
    const escaped = updateSidebar(selected, SidebarPressedKey({ key: 'Escape', itemIds }))

    expect(itemIds).toEqual(['playground', 'models', 'settings', 'general', 'billing'])
    expect(focused.focusedItemId).toBe('settings')
    expect(selected.selectedItemId).toBe('settings')
    expect(escaped.open).toBe(false)
  })

  test('tracks submenu open and closed state explicitly', () => {
    const opened = updateSidebar(
      initSidebar(),
      SidebarToggledSubmenu({ submenuId: 'settings', open: true }),
    )
    const closed = updateSidebar(
      opened,
      SidebarToggledSubmenu({ submenuId: 'settings', open: false }),
    )

    expect(opened.openSubmenuIds).toEqual(['settings'])
    expect(opened.closedSubmenuIds).toEqual([])
    expect(closed.openSubmenuIds).toEqual([])
    expect(closed.closedSubmenuIds).toEqual(['settings'])
  })
})

describe('basecoat sidebar view', () => {
  test('renders Basecoat sidebar structure and item data attributes', () => {
    const model = updateSidebar(
      updateSidebar(
        initSidebar({
          selectedItemId: 'models',
          focusedItemId: 'models',
          openSubmenuIds: ['settings'],
        }),
        SidebarFocusedItem({ itemId: 'models' }),
      ),
      SidebarToggledSubmenu({ submenuId: 'settings', open: true }),
    )
    const rendered = renderHtml(
      sidebar({
        id: 'app-sidebar',
        model,
        label: 'Main navigation',
        side: 'right',
        actions: sidebarMessageActions,
        header: { children: ['OpenAgents'] },
        footer: { children: ['Status'] },
        contentAttrs: { className: 'scrollbar-sm' },
        menu: sampleMenu,
      }),
    )

    expect(rendered).toContain('<aside')
    expect(rendered).toContain('id="app-sidebar"')
    expect(rendered).toContain('class="sidebar"')
    expect(rendered).toContain('data-side="right"')
    expect(rendered).toContain('data-sidebar-initialized="true"')
    expect(rendered).toContain('data-initial-open="true"')
    expect(rendered).toContain('data-initial-mobile-open="false"')
    expect(rendered).toContain('data-breakpoint="768"')
    expect(rendered).toContain('aria-hidden="false"')
    expect(rendered).toContain('<nav aria-label="Main navigation">')
    expect(rendered).toContain('<header>OpenAgents</header>')
    expect(rendered).toContain('<section class="scrollbar-sm">')
    expect(rendered).toContain('role="group"')
    expect(rendered).toContain('aria-labelledby="getting-started-label"')
    expect(rendered).toContain('<h3 id="getting-started-label">Getting started</h3>')
    expect(rendered).toContain('href="/playground"')
    expect(rendered).toContain('aria-current="page"')
    expect(rendered).toContain('data-active="true"')
    expect(rendered).toContain('data-variant="outline"')
    expect(rendered).toContain('data-size="sm"')
    expect(rendered).toContain('data-keep-mobile-sidebar-open=""')
    expect(rendered).toContain('data-sidebar-item-id="models"')
    expect(rendered).toContain('data-focused="true"')
    expect(rendered).toContain('tabIndex="0"')
    expect(rendered).toContain('<details id="settings" open>')
    expect(rendered).toContain('<summary data-sidebar-item-id="settings" aria-controls="settings-content"')
    expect(rendered).toContain('<hr role="separator" id="main-separator"></hr>')
    expect(rendered).toContain('<button data-sidebar-item-id="logout" tabIndex="-1" type="button" disabled>')
    expect(rendered).toContain('<footer>Status</footer>')
  })

  test('renders closed sidebar inert state', () => {
    const rendered = renderHtml(
      sidebar({
        model: updateSidebar(initSidebar(), SidebarClosed()),
        children: ['Custom content'],
      }),
    )

    expect(rendered).toContain('aria-hidden="true"')
    expect(rendered).toContain('inert')
    expect(rendered).toContain('Custom content')
  })

  test('collects navigable and mobile-pinned item ids from nested menu data', () => {
    const closed = initSidebar()
    const open = initSidebar({ openSubmenuIds: ['settings'] })

    expect(sidebarItemIds(sampleMenu)).toEqual([
      'playground',
      'models',
      'settings',
      'general',
      'billing',
    ])
    expect(sidebarVisibleItemIds(closed, sampleMenu)).toEqual([
      'playground',
      'models',
      'settings',
    ])
    expect(sidebarVisibleItemIds(open, sampleMenu)).toEqual([
      'playground',
      'models',
      'settings',
      'general',
      'billing',
    ])
    expect(sidebarKeepMobileOpenItemIds(sampleMenu)).toEqual(['playground'])
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.initSidebar).toBe(initSidebar)
    expect(Basecoat.updateSidebar).toBe(updateSidebar)
    expect(Basecoat.sidebar).toBe(sidebar)
  })
})
