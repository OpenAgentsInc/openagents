import { Scene } from 'foldkit'
import { describe, expect, test } from 'vitest'

import { LoggedOut } from './model'
import {
  SiteCheckoutDemoReturnRoute,
  SiteCheckoutDemoRoute,
} from './route'
import { checkoutDemoScript } from './page/siteCheckoutDemo'
import { update } from './update'
import { view } from './view'

describe('Site checkout demo route', () => {
  test('renders the public demo checkout shell with a real buyer action', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(SiteCheckoutDemoRoute())),
      Scene.expect(Scene.role('heading', { name: 'Demo checkout' }))
        .toExist(),
      Scene.expect(
        Scene.text(
          'Start a demo checkout for an Omega Site product and inspect the clean return status.',
        ),
      ).toExist(),
      Scene.expect(Scene.role('button', { name: 'Start checkout' }))
        .toExist(),
      Scene.expect(Scene.text('Pylon')).not.toExist(),
      Scene.expect(Scene.text('payout')).not.toExist(),
    )
  })

  test('renders clean return routes without checkout query state', () => {
    Scene.scene(
      { update, view },
      Scene.with(
        LoggedOut.init(
          SiteCheckoutDemoReturnRoute({ returnAction: 'status' }),
        ),
      ),
      Scene.expect(Scene.role('heading', { name: 'Demo checkout' }))
        .toExist(),
      Scene.expect(Scene.text('checkout-id')).not.toExist(),
      Scene.expect(Scene.text('raw invoice')).not.toExist(),
    )
  })

  test('uses Site commerce APIs with idempotency and public-safe browser state', () => {
    const script = checkoutDemoScript(SiteCheckoutDemoRoute())

    expect(script).toContain(
      "const discoveryEndpoint = '/api/sites/' + encodeURIComponent(siteId) + '/commerce/discovery';",
    )
    expect(script).toContain('/commerce/checkout-returns/')
    expect(script).toContain("'Idempotency-Key': idempotencyKey()")
    expect(script).toContain(
      "customerDataRefs: item.customerDataRequirementRefs || []",
    )
    expect(script).toContain('Checkout not live')
    expect(script).not.toMatch(
      /\b(lnbc|lntb|lnbcrt|lno1|mnemonic|xprv|payment_preimage|raw_invoice|wallet_secret|private_key|provider payload|payout target)\b/i,
    )
    expect(script).not.toContain('checkout-id=')
  })
})
