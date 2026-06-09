import { describe, expect, test } from 'vitest'

import {
  safeSiteCodeViewerContext,
  siteCodeViewerContextFromElement,
} from './site-code-context'
import { safeSiteElementContext } from './site-element-context'

describe('site code context', () => {
  test('builds read-only source context from a selected element', () => {
    const element = safeSiteElementContext({
      attributes: [
        { name: 'class', value: 'button' },
        { name: 'href', value: '#returns' },
      ],
      selector: 'main a[href="#returns"]',
      tag: 'a',
      text: 'Investment case',
    })

    expect(siteCodeViewerContextFromElement(element!, 'site_version_1')).toEqual(
      {
        language: 'html',
        path: 'selected-element/a.html',
        source: '<a class="button" href="#returns">Investment case</a>',
        versionRef: 'site_version_1',
      },
    )
  })

  test('blocks secret-shaped source before rendering', () => {
    expect(
      safeSiteCodeViewerContext({
        language: 'html',
        path: 'selected-element/div.html',
        source: '<div data-token="secret">Hidden</div>',
        versionRef: 'site_version_1',
      }),
    ).toBeNull()
  })

  test('bounds source and metadata', () => {
    const context = safeSiteCodeViewerContext({
      language: 'HTML'.repeat(20),
      path: `src/${'nested/'.repeat(80)}index.html`,
      source: '<p>Safe</p>'.repeat(400),
      versionRef: 'site_version_long',
    })

    expect(context?.language.length).toBeLessThanOrEqual(40)
    expect(context?.path.length).toBeLessThanOrEqual(160)
    expect(context?.source.length).toBeLessThanOrEqual(1200)
  })
})
