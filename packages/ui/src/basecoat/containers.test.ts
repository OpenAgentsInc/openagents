import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import { button } from './button'
import {
  empty,
  emptyContent,
  emptyDescription,
  emptyFigure,
  emptyFooter,
  emptyHeader,
  emptyMedia,
  emptySection,
  emptyTitle,
  item,
  itemAside,
  itemContent,
  itemDescription,
  itemFigure,
  itemFooter,
  itemGroup,
  itemHeader,
  itemMedia,
  itemSection,
  itemSeparator,
  itemTitle,
} from './containers'
import { renderHtml } from './test-helpers'

describe('basecoat container components', () => {
  test('renders item groups, variants, sizes, and semantic item children', () => {
    const rendered = renderHtml(
      itemGroup({
        className: 'gap-4',
        children: [
          item({
            variant: 'outline',
            role: 'listitem',
            children: [
              itemMedia({ children: ['Icon'] }),
              itemContent({
                children: [
                  itemTitle({ children: ['Security Alert'] }),
                  itemDescription({
                    children: ['New login detected from unknown device.'],
                  }),
                ],
              }),
              itemAside({
                children: [
                  button({
                    variant: 'outline',
                    size: 'sm',
                    children: ['Review'],
                  }),
                ],
              }),
            ],
          }),
          itemSeparator(),
          item({
            href: '/profile',
            size: 'sm',
            role: 'listitem',
            children: [
              itemContent({
                children: [
                  itemTitle({ level: 4, children: ['Profile verified'] }),
                ],
              }),
              itemAside({ children: ['Next'] }),
            ],
          }),
        ],
      }),
    )

    expect(rendered).toContain('<div role="list" class="item-group gap-4">')
    expect(rendered).toContain('<article')
    expect(rendered).toContain('role="listitem"')
    expect(rendered).toContain('class="item"')
    expect(rendered).toContain('data-variant="outline"')
    expect(rendered).toContain('<figure>Icon</figure>')
    expect(rendered).toContain('<section><h3>Security Alert</h3><p>New login detected from unknown device.</p></section>')
    expect(rendered).toContain('<aside><button')
    expect(rendered).toContain('<hr></hr>')
    expect(rendered).toContain('<a')
    expect(rendered).toContain('href="/profile"')
    expect(rendered).toContain('data-size="sm"')
    expect(rendered).toContain('<h4>Profile verified</h4>')
  })

  test('renders item header and footer regions', () => {
    const rendered = renderHtml(
      item({
        variant: 'muted',
        size: 'xs',
        children: [
          itemHeader({ children: ['Preview'] }),
          itemContent({
            children: [itemTitle({ level: 2, children: ['v0-2.0-mini'] })],
          }),
          itemFooter({ children: ['Open Source model for everyone.'] }),
        ],
      }),
    )

    expect(rendered).toContain('data-variant="muted"')
    expect(rendered).toContain('data-size="xs"')
    expect(rendered).toContain('<header>Preview</header>')
    expect(rendered).toContain('<h2>v0-2.0-mini</h2>')
    expect(rendered).toContain('<footer>Open Source model for everyone.</footer>')
  })

  test('renders empty states with header, media, content, and footer', () => {
    const rendered = renderHtml(
      empty({
        className: 'border border-dashed',
        children: [
          emptyHeader({
            children: [
              emptyMedia({ children: ['Folder'] }),
              emptyTitle({ children: ['No Projects Yet'] }),
              emptyDescription({
                children: ['Create your first project to get started.'],
              }),
            ],
          }),
          emptyContent({
            children: ['Search existing projects before creating one.'],
          }),
          emptyFooter({
            children: [
              button({ children: ['Create Project'] }),
              button({
                variant: 'outline',
                children: ['Import Project'],
              }),
            ],
          }),
        ],
      }),
    )

    expect(rendered).toContain('<section class="empty border border-dashed">')
    expect(rendered).toContain('<header><figure>Folder</figure><h3>No Projects Yet</h3><p>Create your first project to get started.</p></header>')
    expect(rendered).toContain('<section>Search existing projects before creating one.</section>')
    expect(rendered).toContain('<footer><button')
    expect(rendered).toContain('Create Project')
    expect(rendered).toContain('data-variant="outline"')
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.itemGroup).toBe(itemGroup)
    expect(Basecoat.item).toBe(item)
    expect(Basecoat.itemMedia).toBe(itemMedia)
    expect(Basecoat.itemFigure).toBe(itemFigure)
    expect(Basecoat.itemContent).toBe(itemContent)
    expect(Basecoat.itemSection).toBe(itemSection)
    expect(Basecoat.itemTitle).toBe(itemTitle)
    expect(Basecoat.itemDescription).toBe(itemDescription)
    expect(Basecoat.itemAside).toBe(itemAside)
    expect(Basecoat.itemHeader).toBe(itemHeader)
    expect(Basecoat.itemFooter).toBe(itemFooter)
    expect(Basecoat.itemSeparator).toBe(itemSeparator)
    expect(Basecoat.empty).toBe(empty)
    expect(Basecoat.emptyHeader).toBe(emptyHeader)
    expect(Basecoat.emptyMedia).toBe(emptyMedia)
    expect(Basecoat.emptyFigure).toBe(emptyFigure)
    expect(Basecoat.emptyTitle).toBe(emptyTitle)
    expect(Basecoat.emptyDescription).toBe(emptyDescription)
    expect(Basecoat.emptyContent).toBe(emptyContent)
    expect(Basecoat.emptySection).toBe(emptySection)
    expect(Basecoat.emptyFooter).toBe(emptyFooter)
  })
})
