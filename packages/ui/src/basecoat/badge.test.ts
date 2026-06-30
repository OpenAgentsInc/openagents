import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import { badge } from './badge'
import { renderHtml } from './test-helpers'

describe('basecoat badge component', () => {
  test('renders Basecoat badge markup with shadcn variants', () => {
    const rendered = renderHtml(
      badge({
        variant: 'secondary',
        children: ['Queued'],
      }),
    )

    expect(rendered).toContain('<span')
    expect(rendered).toContain('class="badge"')
    expect(rendered).toContain('data-variant="secondary"')
    expect(rendered).toContain('Queued')
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.badge).toBe(badge)
  })
})
