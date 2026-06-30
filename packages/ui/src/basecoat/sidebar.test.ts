import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  sidebar,
  sidebarContent,
  sidebarFooter,
  sidebarGroup,
  sidebarGroupLabel,
  sidebarHeader,
  sidebarInit,
  sidebarMenu,
  sidebarMenuButton,
  sidebarMenuItem,
  sidebarMenuLink,
  sidebarNav,
  sidebarSeparator,
  sidebarUpdate,
  sidebarView,
  type SidebarMessage,
} from './sidebar'
import { renderHtml } from './test-helpers'

const message = (input: SidebarMessage): SidebarMessage => input

describe('basecoat sidebar component', () => {
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
