import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import {
  SiteCheckoutDemoPage,
  checkoutDemoScript,
} from './-site-checkout-demo-page'

describe('Start site checkout demo route', () => {
  test('server-renders the public demo checkout shell with a real buyer action', () => {
    const html = renderToStaticMarkup(<SiteCheckoutDemoPage />)

    expect(html).toContain('data-route="site-checkout-demo"')
    expect(html).toContain('data-site-checkout-demo=""')
    expect(html).toContain('Demo checkout')
    expect(html).toContain(
      'Start a demo checkout for an Omega Site product and inspect the clean return status.',
    )
    expect(html).toContain('Start checkout')
    expect(html).not.toContain('Pylon')
    expect(html).not.toContain('payout')
  })

  test('renders clean return routes without checkout query state', () => {
    const html = renderToStaticMarkup(
      <SiteCheckoutDemoPage returnAction="status" />,
    )

    expect(html).toContain('Demo checkout')
    expect(html).toContain('"action":"status"')
    expect(html).not.toContain('checkout-id')
    expect(html).not.toContain('raw invoice')
  })

  test('uses Site commerce APIs with idempotency and public-safe browser state', () => {
    const script = checkoutDemoScript('start')

    expect(script).toContain(
      "const discoveryEndpoint = '/api/sites/' + encodeURIComponent(siteId) + '/commerce/discovery';",
    )
    expect(script).toContain('/commerce/checkout-returns/')
    expect(script).toContain("'Idempotency-Key': idempotencyKey()")
    expect(script).toContain(
      'customerDataRefs: item.customerDataRequirementRefs || []',
    )
    expect(script).toContain('Checkout not live')
    expect(script).not.toMatch(
      /\b(lnbc|lntb|lnbcrt|lno1|mnemonic|xprv|payment_preimage|raw_invoice|wallet_secret|private_key|provider payload|payout target)\b/i,
    )
    expect(script).not.toContain('checkout-id=')
  })
})
