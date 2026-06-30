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
  sidebarContent,
  sidebarFooter,
  sidebarGroup,
  sidebarGroupLabel,
  sidebarHeader,
  sidebarInit,
  sidebarItemIds,
  sidebarKeepMobileOpenItemIds,
  sidebarMenu,
  sidebarMenuButton,
  sidebarMenuItem,
  sidebarMenuLink,
  sidebarMessageActions,
  sidebarNav,
  sidebarSeparator,
  sidebarUpdate,
  sidebarView,
  sidebarVisibleItemIds,
  updateSidebar,
  type SidebarMessage,
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


const message = (input: SidebarMessage): SidebarMessage => input

describe('basecoat sidebar primitive component', () => {
  test('renders Basecoat sidebar markup and donor data attributes', () => {
    const rendered = renderHtml(
      sidebar({
        open: true,
        initialOpen: false,
        initialMobileOpen: true,
        breakpoint: 640,
        side: 'right',
        label: 'Workspace',
        children: [
          sidebarNav({
            label: 'Workspace navigation',
            children: [
              sidebarHeader({ children: ['OpenAgents'] }),
              sidebarContent({
                children: [
                  sidebarGroup({
                    label: 'Projects',
                    children: [
                      sidebarGroupLabel({ children: ['Projects'] }),
                      sidebarMenu({
                        children: [
                          sidebarMenuItem({
                            children: [
                              sidebarMenuLink({
                                href: '/projects/active',
                                value: 'active',
                                selected: true,
                                focused: true,
                                children: ['Active'],
                              }),
                            ],
                          }),
                        ],
                      }),
                    ],
                  }),
                  sidebarSeparator(),
                ],
              }),
              sidebarFooter({ children: ['Signed in'] }),
            ],
          }),
        ],
      }),
    )

    expect(rendered).toContain('<aside')
    expect(rendered).toContain('class="sidebar"')
    expect(rendered).toContain('aria-hidden="false"')
    expect(rendered).toContain('data-initial-open="false"')
    expect(rendered).toContain('data-initial-mobile-open="true"')
    expect(rendered).toContain('data-breakpoint="640"')
    expect(rendered).toContain('data-side="right"')
    expect(rendered).toContain('<nav')
    expect(rendered).toContain('data-slot="sidebar-header"')
    expect(rendered).toContain('data-slot="sidebar-content"')
    expect(rendered).toContain('role="group"')
    expect(rendered).toContain('data-slot="sidebar-group-label"')
    expect(rendered).toContain('data-slot="sidebar-menu"')
    expect(rendered).toContain('href="/projects/active"')
    expect(rendered).toContain('aria-current="page"')
    expect(rendered).toContain('data-active="true"')
    expect(rendered).toContain('tabIndex="0"')
    expect(rendered).toContain('data-slot="sidebar-separator"')
    expect(rendered).toContain('data-slot="sidebar-footer"')
  })

  test('closed sidebars are hidden and inert', () => {
    const rendered = renderHtml(
      sidebar({
        open: false,
        children: [sidebarNav({ children: ['Navigation'] })],
      }),
    )

    expect(rendered).toContain('aria-hidden="true"')
    expect(rendered).toContain('inert')
  })

  test('renders disabled buttons and keep-mobile-open controls', () => {
    const rendered = renderHtml(
      sidebarMenuButton({
        value: 'settings',
        disabled: true,
        keepMobileOpen: true,
        children: ['Settings'],
      }),
    )

    expect(rendered).toContain('<button')
    expect(rendered).toContain('type="button"')
    expect(rendered).toContain('data-value="settings"')
    expect(rendered).toContain('aria-disabled="true"')
    expect(rendered).toContain('data-disabled="true"')
    expect(rendered).toContain('disabled')
    expect(rendered).toContain('data-keep-mobile-sidebar-open')
  })

  test('initializes desktop and mobile open state from breakpoint inputs', () => {
    const desktop = sidebarInit({
      items: [{ value: 'home' }],
      initialOpen: true,
      initialMobileOpen: false,
      breakpoint: 768,
      viewportWidth: 1024,
    })
    const mobile = sidebarInit({
      items: [{ value: 'home' }],
      initialOpen: true,
      initialMobileOpen: false,
      breakpoint: 768,
      viewportWidth: 375,
    })

    expect(desktop.open).toBe(true)
    expect(mobile.open).toBe(false)
    expect(desktop.focusedValue).toBe('home')
  })

  test('updates open state and closes for backdrop clicks', () => {
    const model = sidebarInit({ items: [{ value: 'home' }] })

    expect(sidebarUpdate(model, { _tag: 'SidebarClosed' }).open).toBe(false)
    expect(sidebarUpdate({ ...model, open: false }, { _tag: 'SidebarOpened' }).open).toBe(true)
    expect(sidebarUpdate(model, { _tag: 'SidebarToggled' }).open).toBe(false)
    expect(sidebarUpdate(model, { _tag: 'SidebarBackdropClicked' }).open).toBe(false)
  })

  test('selects activated items and closes mobile unless kept open', () => {
    const model = sidebarInit({
      items: [{ value: 'home' }, { value: 'settings' }],
      initialOpen: true,
    })

    const closed = sidebarUpdate(model, {
      _tag: 'SidebarItemActivated',
      value: 'home',
      mobile: true,
    })
    expect(closed.selectedValue).toBe('home')
    expect(closed.focusedValue).toBe('home')
    expect(closed.open).toBe(false)

    const kept = sidebarUpdate(model, {
      _tag: 'SidebarItemActivated',
      value: 'settings',
      mobile: true,
      keepMobileOpen: true,
    })
    expect(kept.selectedValue).toBe('settings')
    expect(kept.open).toBe(true)
  })

  test('ignores disabled item focus and selection', () => {
    const model = sidebarInit({
      items: [{ value: 'home' }, { value: 'disabled', disabled: true }],
      selectedValue: 'disabled',
    })

    expect(model.selectedValue).toBeNull()
    expect(sidebarUpdate(model, {
      _tag: 'SidebarSelected',
      value: 'disabled',
    })).toEqual(model)
    expect(sidebarUpdate(model, {
      _tag: 'SidebarFocused',
      value: 'disabled',
    })).toEqual(model)
  })

  test('moves focus with sidebar keyboard navigation and closes on Escape', () => {
    const model = sidebarInit({
      items: [
        { value: 'home' },
        { value: 'disabled', disabled: true },
        { value: 'settings' },
      ],
    })

    const next = sidebarUpdate(model, {
      _tag: 'SidebarKeyDown',
      value: 'home',
      key: 'ArrowDown',
    })
    expect(next.focusedValue).toBe('settings')

    const home = sidebarUpdate(next, {
      _tag: 'SidebarKeyDown',
      value: 'settings',
      key: 'Home',
    })
    expect(home.focusedValue).toBe('home')

    const selected = sidebarUpdate(home, {
      _tag: 'SidebarKeyDown',
      value: 'settings',
      key: 'Enter',
    })
    expect(selected.selectedValue).toBe('settings')

    const closed = sidebarUpdate(selected, {
      _tag: 'SidebarKeyDown',
      value: 'settings',
      key: 'Escape',
    })
    expect(closed.open).toBe(false)
  })

  test('sidebarView wires model state into rendered attrs', () => {
    const model = sidebarInit({
      items: [{ value: 'home' }, { value: 'settings' }],
      selectedValue: 'settings',
      focusedValue: 'settings',
    })

    const rendered = renderHtml(
      sidebarView({
        model,
        label: 'Application',
        side: 'left',
        toMessage: message,
        groups: [
          {
            label: 'Main',
            items: [
              { value: 'home', href: '/home', children: ['Home'] },
              { value: 'settings', children: ['Settings'] },
            ],
          },
        ],
      }),
    )

    expect(rendered).toContain('class="sidebar"')
    expect(rendered).toContain('data-side="left"')
    expect(rendered).toContain('data-breakpoint="768"')
    expect(rendered).toContain('aria-label="Application"')
    expect(rendered).toContain('href="/home"')
    expect(rendered).toContain('data-value="settings"')
    expect(rendered).toContain('aria-current="page"')
    expect(rendered).toContain('tabIndex="0"')
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.sidebar).toBe(sidebar)
    expect(Basecoat.sidebarNav).toBe(sidebarNav)
    expect(Basecoat.sidebarHeader).toBe(sidebarHeader)
    expect(Basecoat.sidebarContent).toBe(sidebarContent)
    expect(Basecoat.sidebarFooter).toBe(sidebarFooter)
    expect(Basecoat.sidebarGroup).toBe(sidebarGroup)
    expect(Basecoat.sidebarGroupLabel).toBe(sidebarGroupLabel)
    expect(Basecoat.sidebarMenu).toBe(sidebarMenu)
    expect(Basecoat.sidebarMenuItem).toBe(sidebarMenuItem)
    expect(Basecoat.sidebarMenuButton).toBe(sidebarMenuButton)
    expect(Basecoat.sidebarMenuLink).toBe(sidebarMenuLink)
    expect(Basecoat.sidebarSeparator).toBe(sidebarSeparator)
    expect(Basecoat.sidebarView).toBe(sidebarView)
    expect(Basecoat.sidebarInit).toBe(sidebarInit)
    expect(Basecoat.sidebarUpdate).toBe(sidebarUpdate)
  })
})
