import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  tabs,
  tabsInitialModel,
  tabsKeyboardTarget,
  tabsUpdate,
  type TabsMessage,
} from './tabs'
import { renderHtml } from './test-helpers'

const tabItems = [
  {
    value: 'account',
    tab: ['Account'],
    panel: ['Manage account settings.'],
  },
  {
    value: 'password',
    tab: ['Password'],
    panel: ['Change password.'],
  },
  {
    value: 'disabled',
    tab: ['Disabled'],
    panel: ['Disabled panel.'],
    disabled: true,
  },
] as const

describe('basecoat tabs component', () => {
  test('renders Basecoat tabs markup with selected panel state', () => {
    const rendered = renderHtml(
      tabs<TabsMessage>({
        id: 'settings-tabs',
        ariaLabel: 'Settings sections',
        variant: 'line',
        model: { selectedValue: 'password' },
        items: tabItems,
        toMessage: message => message,
      }),
    )

    expect(rendered).toContain('<div id="settings-tabs" class="tabs">')
    expect(rendered).toContain('<nav role="tablist" aria-orientation="horizontal" aria-label="Settings sections" data-variant="line">')
    expect(rendered).toContain('type="button"')
    expect(rendered).toContain('role="tab"')
    expect(rendered).toContain('id="settings-tabs-tab-2"')
    expect(rendered).toContain('aria-controls="settings-tabs-panel-2"')
    expect(rendered).toContain('aria-selected="true"')
    expect(rendered).toContain('tabIndex="0"')
    expect(rendered).toContain('id="settings-tabs-panel-2"')
    expect(rendered).toContain('aria-labelledby="settings-tabs-tab-2"')
    expect(rendered).toContain('Change password.')
    expect(rendered).toContain('id="settings-tabs-panel-1" tabIndex="-1" hidden')
    expect(rendered).toContain('disabled')
  })

  test('selects the first enabled tab when the default is disabled', () => {
    expect(
      tabsInitialModel({
        items: tabItems,
        defaultValue: 'disabled',
      }),
    ).toEqual({ selectedValue: 'account' })
  })

  test('updates selection and ignores disabled tabs', () => {
    const model = tabsInitialModel({ items: tabItems })

    expect(
      tabsUpdate(
        { items: tabItems },
        model,
        { _tag: 'SelectTab', value: 'password' },
      ),
    ).toEqual({ selectedValue: 'password' })

    expect(
      tabsUpdate(
        { items: tabItems },
        model,
        { _tag: 'SelectTab', value: 'disabled' },
      ),
    ).toEqual({ selectedValue: 'account' })
  })

  test('moves selection and focus with keyboard navigation', () => {
    expect(
      tabsKeyboardTarget(
        { items: tabItems, orientation: 'horizontal' },
        'password',
        'ArrowRight',
      ),
    ).toBe('account')

    expect(
      tabsUpdate(
        { items: tabItems, orientation: 'vertical' },
        { selectedValue: 'account' },
        { _tag: 'KeyDown', value: 'account', key: 'ArrowDown' },
      ),
    ).toEqual({ selectedValue: 'password', focusedValue: 'password' })

    expect(
      tabsUpdate(
        { items: tabItems, orientation: 'vertical' },
        { selectedValue: 'password' },
        { _tag: 'KeyDown', value: 'password', key: 'Home' },
      ),
    ).toEqual({ selectedValue: 'account', focusedValue: 'account' })
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.tabs).toBe(tabs)
    expect(Basecoat.tabsInitialModel).toBe(tabsInitialModel)
    expect(Basecoat.tabsUpdate).toBe(tabsUpdate)
  })
})
