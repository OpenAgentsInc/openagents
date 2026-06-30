import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  command,
  commandDialog,
  commandFocused,
  commandInput,
  commandItem,
  commandKeyDown,
  commandKeyMessage,
  commandMenu,
  commandQueryChanged,
  commandSelected,
  commandShortcut,
  commandTrigger,
  commandView,
  commandVisibleItems,
  initCommand,
  updateCommand,
  type CommandMessage,
} from './command'
import { renderHtml } from './test-helpers'

const items = [
  { value: 'calendar', label: 'Calendar', keywords: ['date', 'event'] },
  { value: 'emoji', label: 'Search Emoji', keywords: ['smile', 'reaction'] },
  { value: 'calculator', label: 'Calculator', disabled: true },
  { value: 'settings', label: 'Settings', force: true },
] as const

const message = (input: CommandMessage): CommandMessage => input

describe('basecoat command component', () => {
  test('initializes active item from the first enabled visible item', () => {
    const model = initCommand({ items })

    expect(model.open).toBe(false)
    expect(model.query).toBe('')
    expect(model.activeValue).toBe('calendar')
    expect(commandVisibleItems(model).map(item => item.value)).toEqual([
      'calendar',
      'emoji',
      'settings',
    ])
  })

  test('filters items by text, keywords, forced items, and manual visibility', () => {
    const filtered = updateCommand(
      initCommand({ items, open: true }),
      commandQueryChanged('smil'),
    )
    expect(commandVisibleItems(filtered).map(item => item.value)).toEqual([
      'emoji',
      'settings',
    ])
    expect(filtered.activeValue).toBe('emoji')

    const manual = initCommand({
      manualFilter: true,
      items: [
        { value: 'remote-a', label: 'Remote A' },
        { value: 'remote-b', label: 'Remote B', hidden: true },
      ],
    })
    expect(commandVisibleItems(manual).map(item => item.value)).toEqual([
      'remote-a',
    ])
  })

  test('models focus, keyboard navigation, selection, and close state', () => {
    const model = initCommand({ items, open: true })
    const focused = updateCommand(model, commandFocused('emoji'))
    expect(focused.activeValue).toBe('emoji')

    const next = updateCommand(focused, commandKeyDown('ArrowDown'))
    expect(next.activeValue).toBe('settings')

    const clamped = updateCommand(next, commandKeyDown('ArrowDown'))
    expect(clamped.activeValue).toBe('settings')

    const home = updateCommand(clamped, commandKeyDown('Home'))
    expect(home.activeValue).toBe('calendar')

    const selected = updateCommand(home, commandSelected('calendar'))
    expect(selected.selectedValue).toBe('calendar')
    expect(selected.open).toBe(false)

    const keptOpen = updateCommand(
      { ...home, open: true },
      commandSelected('emoji', true),
    )
    expect(keptOpen.selectedValue).toBe('emoji')
    expect(keptOpen.open).toBe(true)
  })

  test('maps supported key names to command messages', () => {
    expect(commandKeyMessage('ArrowDown')).toEqual(commandKeyDown('ArrowDown'))
    expect(commandKeyMessage('Enter')).toEqual(commandKeyDown('Enter'))
    expect(commandKeyMessage('Tab')).toBeNull()
  })

  test('renders Basecoat command markup and interactive state attrs', () => {
    const model = updateCommand(
      initCommand({ items, open: true }),
      commandQueryChanged('emoji'),
    )
    const rendered = renderHtml(
      commandDialog({
        model,
        id: 'command-basic',
        ariaLabel: 'Command menu',
        onMessage: message,
        children: [
          command({
            model,
            className: 'border',
            children: [
              commandInput({
                model,
                onMessage: message,
                id: 'command-basic-input',
                menuId: 'command-basic-menu',
                placeholder: 'Type a command or search...',
              }),
              commandMenu({
                id: 'command-basic-menu',
                empty: 'No results found.',
                children: [
                  commandItem({
                    model,
                    value: 'emoji',
                    filter: 'Search Emoji',
                    keywords: ['smile', 'reaction'],
                    onMessage: message,
                    children: ['Search Emoji'],
                  }),
                  commandItem({
                    model,
                    value: 'calendar',
                    filter: 'Calendar',
                    onMessage: message,
                    children: ['Calendar'],
                  }),
                  commandItem({
                    model,
                    value: 'settings',
                    filter: 'Settings',
                    keepOpen: true,
                    onMessage: message,
                    children: ['Settings', commandShortcut({ children: ['cmd+s'] })],
                  }),
                ],
              }),
            ],
          }),
        ],
      }),
    )

    expect(rendered).toContain('<dialog')
    expect(rendered).toContain('class="command-dialog"')
    expect(rendered).toContain('open=""')
    expect(rendered).toContain('class="command border"')
    expect(rendered).toContain('<header')
    expect(rendered).toContain('role="combobox"')
    expect(rendered).toContain('aria-controls="command-basic-menu"')
    expect(rendered).toContain('aria-activedescendant="emoji"')
    expect(rendered).toContain('role="menu"')
    expect(rendered).toContain('data-empty="No results found."')
    expect(rendered).toContain('role="menuitem"')
    expect(rendered).toContain('class="active"')
    expect(rendered).toContain('aria-hidden="true"')
    expect(rendered).toContain('data-keep-command-open=""')
    expect(rendered).toContain('data-shortcut=""')
  })

  test('commandView wires grouped items from the model', () => {
    const model = initCommand({
      items,
      selectedValue: 'settings',
    })
    const rendered = renderHtml(
      commandView({
        model,
        toMessage: message,
        inputId: 'command-input',
        menuId: 'command-menu',
        placeholder: 'Search commands',
        groups: [
          {
            id: 'suggestions',
            heading: ['Suggestions'],
            items: [
              { value: 'calendar', label: 'Calendar', children: ['Calendar'] },
              { value: 'emoji', label: 'Search Emoji', children: ['Search Emoji'] },
            ],
          },
          {
            id: 'settings',
            heading: ['Settings'],
            items: [
              {
                value: 'settings',
                label: 'Settings',
                force: true,
                children: ['Settings'],
                shortcut: ['cmd+s'],
              },
            ],
          },
        ],
      }),
    )

    expect(rendered).toContain('role="group"')
    expect(rendered).toContain('role="heading"')
    expect(rendered).toContain('aria-labelledby="suggestions"')
    expect(rendered).toContain('<hr role="separator"')
    expect(rendered).toContain('data-selected="true"')
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.command).toBe(command)
    expect(Basecoat.commandDialog).toBe(commandDialog)
    expect(Basecoat.commandTrigger).toBe(commandTrigger)
    expect(Basecoat.commandInput).toBe(commandInput)
    expect(Basecoat.commandMenu).toBe(commandMenu)
    expect(Basecoat.commandItem).toBe(commandItem)
    expect(Basecoat.commandView).toBe(commandView)
    expect(Basecoat.initCommand).toBe(initCommand)
    expect(Basecoat.updateCommand).toBe(updateCommand)
  })
})
