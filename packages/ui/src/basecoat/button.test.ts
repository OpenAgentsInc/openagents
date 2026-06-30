import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import { button, buttonGroup, kbd } from './button'
import { renderHtml } from './test-helpers'

describe('basecoat button components', () => {
  test('renders shadcn button variants and sizes as Basecoat data attributes', () => {
    const rendered = renderHtml(
      button({
        variant: 'destructive',
        size: 'lg',
        children: ['Delete'],
      }),
    )

    expect(rendered).toContain('<button')
    expect(rendered).toContain('class="btn"')
    expect(rendered).toContain('type="button"')
    expect(rendered).toContain('data-variant="destructive"')
    expect(rendered).toContain('data-size="lg"')
    expect(rendered).toContain('Delete')
  })

  test('renders button groups and keyboard tokens', () => {
    const rendered = renderHtml(
      buttonGroup({
        orientation: 'vertical',
        children: [
          button({ variant: 'outline', children: ['Save'] }),
          kbd({ children: ['Cmd', 'S'] }),
        ],
      }),
    )

    expect(rendered).toContain('class="button-group"')
    expect(rendered).toContain('data-orientation="vertical"')
    expect(rendered).toContain('data-variant="outline"')
    expect(rendered).toContain('<kbd class="kbd">')
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.button).toBe(button)
    expect(Basecoat.buttonGroup).toBe(buttonGroup)
    expect(Basecoat.kbd).toBe(kbd)
  })
})
