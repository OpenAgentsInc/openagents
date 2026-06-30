import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  breadcrumb,
  breadcrumbEllipsis,
  breadcrumbItem,
  breadcrumbLink,
  breadcrumbList,
  breadcrumbPage,
  breadcrumbSeparator,
} from './breadcrumb'
import { renderHtml } from './test-helpers'

describe('basecoat breadcrumb components', () => {
  test('renders Basecoat breadcrumb landmark, list, links, separators, and current page', () => {
    const rendered = renderHtml(
      breadcrumb({
        children: [
          breadcrumbList({
            children: [
              breadcrumbItem({
                children: [
                  breadcrumbLink({ href: '/', children: ['Home'] }),
                ],
              }),
              breadcrumbSeparator({}),
              breadcrumbItem({
                children: [
                  breadcrumbLink({
                    href: '/components',
                    children: ['Components'],
                  }),
                ],
              }),
              breadcrumbSeparator({}),
              breadcrumbItem({
                children: [
                  breadcrumbPage({ children: ['Breadcrumb'] }),
                ],
              }),
            ],
          }),
        ],
      }),
    )

    expect(rendered).toContain('<nav')
    expect(rendered).toContain('class="breadcrumb"')
    expect(rendered).toContain('aria-label="Breadcrumb"')
    expect(rendered).toContain('<ol>')
    expect(rendered).toContain('<li><a href="/">Home</a></li>')
    expect(rendered).toContain('href="/components"')
    expect(rendered).toContain('aria-hidden="true"')
    expect(rendered).toContain('data-rtl-flip=""')
    expect(rendered).toContain('class="lucide lucide-chevron-right"')
    expect(rendered).toContain(
      '<li><span aria-current="page">Breadcrumb</span></li>',
    )
  })

  test('renders custom separators, collapsed ellipsis, and rtl direction', () => {
    const rendered = renderHtml(
      breadcrumb({
        label: 'Ignored when ariaLabel is present',
        dir: 'rtl',
        ariaLabel: 'Trail',
        className: 'text-sm',
        children: [
          breadcrumbList({
            children: [
              breadcrumbItem({
                children: [
                  breadcrumbLink({
                    href: '/docs',
                    rel: 'external',
                    target: '_blank',
                    children: ['Docs'],
                  }),
                ],
              }),
              breadcrumbSeparator({ children: ['/'] }),
              breadcrumbEllipsis({ label: 'More documentation pages' }),
              breadcrumbSeparator({ rtlFlip: false }),
              breadcrumbItem({
                children: [
                  breadcrumbPage({
                    className: 'font-medium',
                    current: 'step',
                    children: ['API'],
                  }),
                ],
              }),
            ],
          }),
        ],
      }),
    )

    expect(rendered).toContain('class="breadcrumb text-sm"')
    expect(rendered).toContain('aria-label="Trail"')
    expect(rendered).toContain('dir="rtl"')
    expect(rendered).toContain('rel="external"')
    expect(rendered).toContain('target="_blank"')
    expect(rendered).toContain('<li aria-hidden="true">/</li>')
    expect(rendered).toContain('<li><span aria-hidden="true">')
    expect(rendered).toContain('class="lucide lucide-ellipsis"')
    expect(rendered).toContain(
      '<span class="sr-only">More documentation pages</span>',
    )
    expect(rendered).toContain('<li aria-hidden="true"><svg')
    expect(rendered).toContain(
      '<span aria-current="step" class="font-medium">API</span>',
    )
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.breadcrumb).toBe(breadcrumb)
    expect(Basecoat.breadcrumbList).toBe(breadcrumbList)
    expect(Basecoat.breadcrumbItem).toBe(breadcrumbItem)
    expect(Basecoat.breadcrumbLink).toBe(breadcrumbLink)
    expect(Basecoat.breadcrumbPage).toBe(breadcrumbPage)
    expect(Basecoat.breadcrumbSeparator).toBe(breadcrumbSeparator)
    expect(Basecoat.breadcrumbEllipsis).toBe(breadcrumbEllipsis)
  })
})
