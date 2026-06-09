import { describe, expect, test } from 'vitest'

import {
  SITE_ELEMENT_TARGET_MESSAGE_TYPE,
  siteElementContextFromBridgePayload,
} from './site-preview-bridge'

describe('site preview bridge', () => {
  test('accepts valid postMessage element-target payloads', () => {
    expect(
      siteElementContextFromBridgePayload({
        attributes: [
          { name: 'class', value: 'button' },
          { name: 'href', value: '#returns' },
        ],
        selector: 'main a[href="#returns"]',
        tag: 'a',
        text: 'Investment case',
        type: SITE_ELEMENT_TARGET_MESSAGE_TYPE,
      }),
    ).toMatchObject({
      htmlSnippet: '<a class="button" href="#returns">Investment case</a>',
      selector: 'main a[href="#returns"]',
      tag: 'a',
    })
  })

  test('rejects wrong message types and missing selectors', () => {
    expect(
      siteElementContextFromBridgePayload({
        selector: 'main a',
        tag: 'a',
        type: 'unknown',
      }),
    ).toBeNull()
    expect(
      siteElementContextFromBridgePayload({
        tag: 'a',
        type: SITE_ELEMENT_TARGET_MESSAGE_TYPE,
      }),
    ).toBeNull()
  })

  test('rejects unsafe postMessage payloads through the element sanitizer', () => {
    expect(
      siteElementContextFromBridgePayload({
        selector: 'main [data-token="secret"]',
        tag: 'div',
        text: 'Secret',
        type: SITE_ELEMENT_TARGET_MESSAGE_TYPE,
      }),
    ).toBeNull()
  })
})
