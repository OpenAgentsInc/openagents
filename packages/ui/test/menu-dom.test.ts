import { describe, expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import {
  BASECOAT_CONTEXT_MENU_MENU_CLASS,
  BASECOAT_CONTEXT_MENU_POPOVER_CLASS,
  BASECOAT_CONTEXT_MENU_ROOT_CLASS,
  createBasecoatContextMenu,
} from '../src/menu-dom'

const here = (rel: string) => fileURLToPath(new URL(rel, import.meta.url))

describe('DOM Basecoat context menu helper', () => {
  test('exports Basecoat dropdown-compatible classes', () => {
    expect(typeof createBasecoatContextMenu).toBe('function')
    expect(BASECOAT_CONTEXT_MENU_ROOT_CLASS).toContain('dropdown-menu')
    expect(BASECOAT_CONTEXT_MENU_ROOT_CLASS).toContain('oa-ui-menu-dom')
    expect(BASECOAT_CONTEXT_MENU_POPOVER_CLASS).toBe('oa-ui-menu-dom-popover')
    expect(BASECOAT_CONTEXT_MENU_MENU_CLASS).toBe('oa-ui-menu-dom-menu')

    const index = readFileSync(here('../src/index.ts'), 'utf8')
    expect(index).toContain("export * from './menu-dom'")
  })

  test('styles the menu through shared OpenAgents tokens', () => {
    const css = readFileSync(here('../src/shared.css'), 'utf8')

    expect(css).toContain('.oa-ui-menu-dom')
    expect(css).toContain('var(--oa-color-component-surface)')
    expect(css).toContain('var(--oa-color-component-border)')
    expect(css).toContain('var(--oa-color-component-text)')
    expect(css).toContain('var(--oa-color-component-surface-active)')
  })
})
