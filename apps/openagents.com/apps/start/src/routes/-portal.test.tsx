// PORTAL-1 (#8652): /portal Effect Native view-program coverage.
//
// Behavior-contract oracles (packages/behavior-contracts/src/openagents-apps.ts):
//   * openagents_web.portal_owner_scoped_engagement.v1 — the surface is
//     login-gated and renders only the caller's own engagement (the server
//     enforces owner scoping; this surface never offers a foreign lookup).
//   * openagents_web.portal_decision_receipts.v1 — approve/reject dispatch
//     typed intents and render the minted decision receipt ref.
import { viewStructure } from '@effect-native/render-dom'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test, vi } from 'vitest'

import { Effect, Exit, Scope, SubscriptionRef } from '@effect-native/core/effect'

import {
  initialPortalPageState,
  mountPortalSurface,
  portalContentPairs,
  portalPageView,
  type PortalPageState,
} from './-portal-core'
import { PortalPage } from './-portal-page'
import type { PortalContentItem } from './-portal-data'

const item = (
  overrides: Partial<PortalContentItem> & { id: string },
): PortalContentItem => ({
  kind: 'post',
  channel: 'linkedin',
  variant: 'a',
  pairRef: null,
  title: 'Title',
  body: 'Body',
  state: 'draft',
  decidedAt: null,
  decisionReceiptRef: null,
  ...overrides,
})

const READY_STATE: PortalPageState = {
  phase: 'ready',
  engagement: {
    id: 'portal_engagement_1',
    name: 'Strategic Consulting Demo',
    status: 'active',
    createdAt: '2026-07-10T00:00:00.000Z',
  },
  items: [
    item({ id: 'item_a', variant: 'a', pairRef: 'pair-1', title: 'Post A' }),
    item({ id: 'item_b', variant: 'b', pairRef: 'pair-1', title: 'Post B' }),
    item({
      id: 'item_c',
      title: 'Approved earlier',
      state: 'approved',
      decidedAt: '2026-07-10T01:00:00.000Z',
      decisionReceiptRef: 'portal_content_decision:pcd_1',
    }),
  ],
  kpis: [
    { key: 'funnel_traffic', label: 'Funnel traffic', value: null, note: 'placeholder until live funnel wiring' },
    { key: 'leads', label: 'Leads', value: null, note: 'placeholder until live funnel wiring' },
    { key: 'conversions', label: 'Conversions', value: null, note: 'placeholder until live funnel wiring' },
  ],
  decisionPanels: {},
}

const waitFor = async (predicate: () => boolean): Promise<void> => {
  await vi.waitFor(() => {
    if (!predicate()) {
      throw new Error('condition not met')
    }
  })
}

const jsonResponse = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  })

describe('PORTAL-1 /portal Effect Native route (#8652)', () => {
  test('server render is only a thin mount shim, not portal-content React', () => {
    const html = renderToStaticMarkup(<PortalPage />)
    expect(html).toContain('data-route="portal"')
    expect(html).toContain('data-portal-root=""')
    expect(html).not.toContain('Content calendar')
    expect(html).not.toContain('Funnel KPIs')
  })

  test('logged-out state renders the login gate, never engagement content', () => {
    const serialized = JSON.stringify(
      portalPageView({ ...initialPortalPageState, phase: 'logged_out' }),
    )
    expect(serialized).toContain('"catalogVersion":"effect-native/v30"')
    expect(serialized).toContain('Log in to view your engagement.')
    expect(serialized).toContain('/login/github?returnTo=%2Fportal')
    expect(serialized).not.toContain('Content calendar')
    expect(serialized).not.toContain('Funnel KPIs')
    expect(serialized).not.toContain('className')
  })

  test('empty state renders the designed "setup is being prepared" card', () => {
    const serialized = JSON.stringify(
      portalPageView({ ...initialPortalPageState, phase: 'empty' }),
    )
    expect(serialized).toContain('Your setup is being prepared')
    expect(serialized).toContain('No engagement is linked to this account yet.')
    expect(serialized).not.toContain('Approve')
  })

  test('ready state is one typed EN tree: header, honest KPI tiles, A/B pairs, decision intents', () => {
    const tree = portalPageView(READY_STATE)
    const structure = viewStructure(tree)
    const serialized = JSON.stringify(tree)

    expect(structure).toMatchObject({ tag: 'Stack', key: 'portal-root' })
    // Engagement header + status badge.
    expect(serialized).toContain('Strategic Consulting Demo')
    expect(serialized).toContain('"_tag":"Badge"')
    // Honest KPI placeholders: em dash values, never fabricated numbers.
    expect(serialized).toContain('"_tag":"StatTile"')
    expect(serialized).toContain('"value":"—"')
    expect(serialized).toContain('Honest placeholders')
    // A/B variants side by side with channel/variant tags.
    expect(serialized).toContain('"_tag":"Chip"')
    expect(serialized).toContain('variant A')
    expect(serialized).toContain('variant B')
    expect(serialized).toContain('Post A')
    expect(serialized).toContain('Post B')
    // Typed decision intents on draft items.
    expect(serialized).toContain('PortalDecisionSubmitted')
    expect(serialized).toContain('"decision":"approve"')
    expect(serialized).toContain('"decision":"reject"')
    // Decided item renders its receipt ref, no buttons.
    expect(serialized).toContain('receipt: portal_content_decision:pcd_1')
    // No React/renderer leakage in the authored tree.
    expect(serialized).not.toContain('className')
  })

  test('portalContentPairs groups A/B variants and keeps unpaired items alone', () => {
    const rows = portalContentPairs(READY_STATE.items)
    expect(rows).toHaveLength(2)
    expect(rows[0]!.map((entry) => entry.id)).toEqual(['item_a', 'item_b'])
    expect(rows[1]!.map((entry) => entry.id)).toEqual(['item_c'])
  })

  test('mount smoke: logged-out fetch renders the login gate in real DOM', async () => {
    const container = document.createElement('div')
    document.body.appendChild(container)

    const fetchFn = (async (input: RequestInfo | URL) => {
      const url = String(input)
      if (url.includes('/api/auth/session')) {
        return jsonResponse({ authenticated: false })
      }
      return jsonResponse({ error: 'not_found' }, 404)
    }) as typeof fetch

    const scope = await Effect.runPromise(Scope.make())
    const surface = await Effect.runPromise(
      Scope.provide(scope)(mountPortalSurface(container, { fetchFn })),
    )

    await waitFor(() =>
      (container.textContent ?? '').includes('Log in to view your engagement.'),
    )
    expect(container.textContent).not.toContain('Content calendar')

    await Effect.runPromise(surface.unmount)
    await Effect.runPromise(Scope.close(scope, Exit.void))
    container.remove()
  })

  test('mount smoke: approve dispatch is optimistic and renders the receipt ref', async () => {
    const container = document.createElement('div')
    document.body.appendChild(container)

    let decisionCalls = 0
    const fetchFn = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input)
      if (url.includes('/api/auth/session')) {
        return jsonResponse({ authenticated: true })
      }
      if (url.includes('/api/portal/engagement')) {
        return jsonResponse({
          engagement: {
            id: 'portal_engagement_1',
            name: 'Strategic Consulting Demo',
            status: 'active',
            createdAt: '2026-07-10T00:00:00.000Z',
          },
          items: [
            {
              id: 'item_a',
              kind: 'post',
              channel: 'linkedin',
              variant: 'a',
              pairRef: 'pair-1',
              title: 'Post A',
              body: 'Body A',
              state: 'draft',
              decidedAt: null,
              decisionReceiptRef: null,
            },
          ],
          kpis: [
            {
              key: 'leads',
              label: 'Leads',
              value: null,
              note: 'placeholder until live funnel wiring',
            },
          ],
        })
      }
      if (url.includes('/decision')) {
        decisionCalls += 1
        expect(init?.method).toBe('POST')
        expect(JSON.parse(String(init?.body))).toEqual({ decision: 'approve' })
        return jsonResponse({
          ok: true,
          item: { state: 'approved' },
          receiptRef: 'portal_content_decision:pcd_live',
          alreadyDecided: false,
        })
      }
      return jsonResponse({ error: 'not_found' }, 404)
    }) as typeof fetch

    const scope = await Effect.runPromise(Scope.make())
    const surface = await Effect.runPromise(
      Scope.provide(scope)(mountPortalSurface(container, { fetchFn })),
    )

    await waitFor(() =>
      (container.textContent ?? '').includes('Strategic Consulting Demo'),
    )
    expect(container.textContent).toContain('Post A')

    const approve = Array.from(container.querySelectorAll('button')).find(
      (button) => button.textContent?.includes('Approve'),
    )
    expect(approve).toBeDefined()
    approve!.click()
    await waitFor(() =>
      (container.textContent ?? '').includes(
        'receipt: portal_content_decision:pcd_live',
      ),
    )

    expect(decisionCalls).toBe(1)
    expect(container.textContent).toContain('approved')
    // Buttons are gone once decided.
    expect(
      Array.from(container.querySelectorAll('button')).some((button) =>
        button.textContent?.includes('Approve'),
      ),
    ).toBe(false)

    const state = await Effect.runPromise(SubscriptionRef.get(surface.state))
    expect(state.items[0]?.state).toBe('approved')
    expect(state.items[0]?.decisionReceiptRef).toBe(
      'portal_content_decision:pcd_live',
    )

    await Effect.runPromise(surface.unmount)
    await Effect.runPromise(Scope.close(scope, Exit.void))
    container.remove()
  })

  test('mount smoke: failed decision rolls the item back to draft', async () => {
    const container = document.createElement('div')
    document.body.appendChild(container)

    const fetchFn = (async (input: RequestInfo | URL) => {
      const url = String(input)
      if (url.includes('/api/auth/session')) {
        return jsonResponse({ authenticated: true })
      }
      if (url.includes('/api/portal/engagement')) {
        return jsonResponse({
          engagement: {
            id: 'portal_engagement_1',
            name: 'Demo',
            status: 'active',
            createdAt: '2026-07-10T00:00:00.000Z',
          },
          items: [
            {
              id: 'item_a',
              kind: 'post',
              channel: 'linkedin',
              variant: 'a',
              pairRef: null,
              title: 'Post A',
              body: 'Body A',
              state: 'draft',
              decidedAt: null,
              decisionReceiptRef: null,
            },
          ],
          kpis: [],
        })
      }
      if (url.includes('/decision')) {
        return jsonResponse(
          { error: 'portal_validation_error', reason: 'refused' },
          422,
        )
      }
      return jsonResponse({ error: 'not_found' }, 404)
    }) as typeof fetch

    const scope = await Effect.runPromise(Scope.make())
    const surface = await Effect.runPromise(
      Scope.provide(scope)(mountPortalSurface(container, { fetchFn })),
    )

    await waitFor(() => (container.textContent ?? '').includes('Post A'))
    const reject = Array.from(container.querySelectorAll('button')).find(
      (button) => button.textContent?.includes('Reject'),
    )
    expect(reject).toBeDefined()
    reject!.click()
    await waitFor(() =>
      (container.textContent ?? '').includes('Decision failed'),
    )
    const state = await Effect.runPromise(SubscriptionRef.get(surface.state))
    expect(state.items[0]?.state).toBe('draft')

    await Effect.runPromise(surface.unmount)
    await Effect.runPromise(Scope.close(scope, Exit.void))
    container.remove()
  })

  test('source boundary: EN packages only, no local one-off primitives', () => {
    const source = readFileSync(
      join(process.cwd(), 'src/routes/-portal-core.tsx').replace(
        '-portal-core.tsx',
        '-portal-core.ts',
      ),
      'utf8',
    )
    const routeSource = readFileSync(
      join(process.cwd(), 'src/routes/portal.tsx'),
      'utf8',
    )

    expect(source).toContain("from '@effect-native/core'")
    expect(source).toContain("from '@effect-native/render-dom'")
    expect(source).toContain("from '@effect-native/tokens'")
    for (const symbol of [
      'StatTile',
      'StatusBanner',
      'Badge',
      'Chip',
      'Divider',
      'Card',
      'Section',
      'Stack',
    ]) {
      expect(source).toContain(symbol)
    }
    expect(source).not.toContain('lucide-react')
    expect(source).not.toContain('@/components')
    expect(routeSource).toContain("createFileRoute('/portal')")
  })
})
