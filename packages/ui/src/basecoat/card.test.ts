import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  card,
  cardAction,
  cardDescription,
  cardFooter,
  cardHeader,
  cardSection,
  cardTitle,
} from './card'
import { renderHtml } from './test-helpers'

describe('basecoat card components', () => {
  test('renders Basecoat card slots and small size', () => {
    const rendered = renderHtml(
      card({
        size: 'sm',
        children: [
          cardHeader({
            children: [
              cardTitle({ level: 3, children: ['Agent run'] }),
              cardDescription({ children: ['Proof-ready closeout'] }),
              cardAction({ children: ['Open'] }),
            ],
          }),
          cardSection({ children: ['Body'] }),
          cardFooter({ children: ['Footer'] }),
        ],
      }),
    )

    expect(rendered).toContain('<article')
    expect(rendered).toContain('class="card"')
    expect(rendered).toContain('data-size="sm"')
    expect(rendered).toContain('<header>')
    expect(rendered).toContain('<h3 class="card-title">Agent run</h3>')
    expect(rendered).toContain('<p class="card-description">Proof-ready closeout</p>')
    expect(rendered).toContain('data-slot="card-action"')
    expect(rendered).toContain('<section>Body</section>')
    expect(rendered).toContain('<footer>Footer</footer>')
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.card).toBe(card)
    expect(Basecoat.cardHeader).toBe(cardHeader)
    expect(Basecoat.cardTitle).toBe(cardTitle)
    expect(Basecoat.cardDescription).toBe(cardDescription)
    expect(Basecoat.cardAction).toBe(cardAction)
    expect(Basecoat.cardSection).toBe(cardSection)
    expect(Basecoat.cardFooter).toBe(cardFooter)
  })
})
