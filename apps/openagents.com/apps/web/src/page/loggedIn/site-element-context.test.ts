import { describe, expect, test } from 'vitest'

import {
  safeSiteElementContext,
  siteElementContextDraft,
} from './site-element-context'

describe('site element context', () => {
  test('builds a bounded safe element reference', () => {
    const context = safeSiteElementContext({
      attributes: [
        { name: 'class', value: 'button' },
        { name: 'href', value: '#returns' },
        { name: 'onclick', value: 'steal()' },
      ],
      selector: 'main a[href="#returns"]',
      tag: 'a',
      text: 'Investment case',
    })

    expect(context).toEqual({
      attributes: [
        { name: 'class', value: 'button' },
        { name: 'href', value: '#returns' },
      ],
      htmlSnippet: '<a class="button" href="#returns">Investment case</a>',
      selector: 'main a[href="#returns"]',
      tag: 'a',
      text: 'Investment case',
    })
    expect(siteElementContextDraft(context!)).toContain(
      '<a class="button" href="#returns">Investment case</a>',
    )
  })

  test('rejects secret-shaped selectors and attributes', () => {
    expect(
      safeSiteElementContext({
        selector: 'main [data-token="secret"]',
        tag: 'div',
        text: 'Billing',
      }),
    ).toBeNull()

    expect(
      safeSiteElementContext({
        attributes: [{ name: 'title', value: 'bearer abc123' }],
        selector: 'main .billing',
        tag: 'div',
        text: 'Billing',
      })?.attributes,
    ).toEqual([])
  })

  test('bounds long text and snippets', () => {
    const context = safeSiteElementContext({
      attributes: [{ name: 'class', value: 'x'.repeat(200) }],
      selector: `main ${'.very-long'.repeat(40)}`,
      tag: 'p',
      text: 'Evidence '.repeat(80),
    })

    expect(context?.selector.length).toBeLessThanOrEqual(160)
    expect(context?.text?.length).toBeLessThanOrEqual(180)
    expect(context?.htmlSnippet.length).toBeLessThanOrEqual(264)
  })
})
