// PORTAL-1 (#8652): /portal client-portal surface coverage.
//
// Behavior-contract oracles (packages/behavior-contracts/src/openagents-apps.ts):
//   * openagents_web.portal_owner_scoped_engagement.v1 — the surface is
//     login-gated and renders only the caller's own engagement (the server
//     enforces owner scoping; this surface never offers a foreign lookup).
//   * openagents_web.portal_decision_receipts.v1 — approve/reject post the
//     decision and render the minted decision receipt ref.
//   * openagents_web.portal_empty_state_account_identity.v1 — the
//     authenticated empty state always names the signed-in account
//     (email → login → honest fallback) and offers a sign-out/switch-account
//     affordance (#8652 reopen: owner hit a mismatched-email binding blind).
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { act } from 'react'
import { type Root, createRoot } from 'react-dom/client'
import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, describe, expect, test } from 'vitest'

import {
  initialPortalPageState,
  portalContentPairs,
  type PortalPageState,
} from './-portal-core'
import { PortalPage, PortalSurface } from './-portal-page'
import type { PortalContentItem } from './-portal-data'

;(
  globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true

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
  identity: { email: 'client@example.com', login: 'client' },
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

/** Every string the login gate and the loading phase must never leak. */
const ENGAGEMENT_CONTENT = [
  'Content calendar',
  'Funnel KPIs',
  'Strategic Consulting Demo',
  'Approve',
  'Reject',
  'receipt:',
] as const

const jsonResponse = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  })

let root: Root | null = null
let container: HTMLDivElement | null = null
const originalFetch = globalThis.fetch

afterEach(async () => {
  if (root !== null) await act(async () => root?.unmount())
  container?.remove()
  root = null
  container = null
  globalThis.fetch = originalFetch
})

/** Flush enough microtask turns for a fetch + JSON parse chain to settle. */
const flush = async (): Promise<void> => {
  for (let turn = 0; turn < 8; turn += 1) {
    await act(async () => {})
  }
}

const mount = async (fetchFn: typeof fetch): Promise<HTMLDivElement> => {
  globalThis.fetch = fetchFn
  container = document.createElement('div')
  document.body.appendChild(container)
  root = createRoot(container)
  await act(async () => root?.render(<PortalPage />))
  await flush()
  return container
}

describe('PORTAL-1 /portal route (#8652)', () => {
  test('server render carries the portal surface, and it is the pre-session loading phase', () => {
    // Converted from an Effect Native mount shim to plain React (#9325): the
    // portal now renders on the server. The server has no session, so the
    // server-rendered document is always the loading phase.
    const html = renderToStaticMarkup(<PortalPage />)
    expect(html).toContain('data-route="portal"')
    expect(html).toContain('data-portal-root=""')
    expect(html).toContain('aria-label="OpenAgents client portal"')
    expect(html).toContain('data-portal-phase="loading"')
    expect(html).toContain('Loading your portal…')
    for (const leak of ENGAGEMENT_CONTENT) {
      expect(html).not.toContain(leak)
    }
  })

  test('server-rendered logged-out phase is the login gate ONLY, never engagement content', () => {
    // Login-gate privacy (openagents_web.portal_owner_scoped_engagement.v1):
    // now that the surface server-renders, the logged-out document itself
    // must carry no engagement content at all.
    const html = renderToStaticMarkup(
      <PortalSurface
        state={{ ...initialPortalPageState, phase: 'logged_out' }}
      />,
    )
    expect(html).toContain('data-portal-phase="logged_out"')
    expect(html).toContain('Client portal')
    expect(html).toContain('Log in to view your engagement.')
    expect(html).toContain(
      'Your engagement dashboard, content calendar, and approval queue are private to your account.',
    )
    expect(html).toContain('Log in with GitHub')
    expect(html).toContain('href="/login/github?returnTo=%2Fportal"')
    for (const leak of ENGAGEMENT_CONTENT) {
      expect(html).not.toContain(leak)
    }
  })

  test('server-rendered logged-out phase carries no item, identity, or receipt markup', () => {
    const html = renderToStaticMarkup(
      <PortalSurface
        state={{
          ...initialPortalPageState,
          phase: 'logged_out',
          // Even if a stale identity/engagement ever rode along, the gate
          // phase must render none of it.
          identity: { email: 'client@example.com', login: 'client' },
          engagement: READY_STATE.engagement,
          items: READY_STATE.items,
          kpis: READY_STATE.kpis,
        }}
      />,
    )
    expect(html).not.toContain('data-portal-item')
    expect(html).not.toContain('data-portal-kpi')
    expect(html).not.toContain('client@example.com')
    expect(html).not.toContain('portal_content_decision')
    expect(html).not.toContain('Post A')
  })

  test('empty state shows WHO is signed in and a switch-account affordance (#8652 reopen)', () => {
    // Owner-reported failure 2026-07-10: authenticated /portal with a
    // mismatched engagement binding rendered only "setup is being prepared"
    // with no account context and no way out. The empty state must always
    // name the signed-in identity and offer sign-out/switch-account.
    const html = renderToStaticMarkup(
      <PortalSurface
        state={{
          ...initialPortalPageState,
          phase: 'empty',
          identity: { email: 'chris@openagents.com', login: 'AtlantisPleb' },
        }}
      />,
    )
    expect(html).toContain('Your setup is being prepared')
    expect(html).toContain(
      'No engagement is linked to this account yet. Signed in as chris@openagents.com.',
    )
    expect(html).toContain('different email')
    expect(html).toContain('Sign out / switch account')
    expect(html).toContain('href="/logout"')
    expect(html).not.toContain('Approve')
  })

  test('empty state never renders a blank identity (email → login → honest fallback)', () => {
    const loginOnly = renderToStaticMarkup(
      <PortalSurface
        state={{
          ...initialPortalPageState,
          phase: 'empty',
          identity: { email: null, login: 'AtlantisPleb' },
        }}
      />,
    )
    expect(loginOnly).toContain('Signed in as AtlantisPleb.')

    const noIdentity = renderToStaticMarkup(
      <PortalSurface state={{ ...initialPortalPageState, phase: 'empty' }} />,
    )
    expect(noIdentity).toContain(
      'Signed in as your account (no email on this session).',
    )
    expect(noIdentity).toContain('Sign out / switch account')
  })

  test('unavailable state is honest and shows no stale or fabricated data', () => {
    const html = renderToStaticMarkup(
      <PortalSurface
        state={{ ...initialPortalPageState, phase: 'unavailable' }}
      />,
    )
    expect(html).toContain('Portal unavailable')
    expect(html).toContain(
      'The portal API is unreachable right now. Nothing is shown rather than showing stale or fabricated data.',
    )
    for (const leak of ENGAGEMENT_CONTENT) {
      expect(html).not.toContain(leak)
    }
  })

  test('ready state renders header, honest KPI tiles, A/B pairs, and decision affordances', () => {
    const html = renderToStaticMarkup(<PortalSurface state={READY_STATE} />)

    // Engagement header + status badge.
    expect(html).toContain('Strategic Consulting Demo')
    expect(html).toContain('data-slot="badge"')
    expect(html).toContain('active')
    expect(html).toContain(
      'Your engagement at a glance: funnel status, the content calendar, and your approval queue.',
    )
    // Honest KPI placeholders: em dash values, never fabricated numbers.
    expect(html).toContain('Funnel KPIs')
    expect(html).toContain('data-portal-kpi="funnel_traffic"')
    expect(html).toContain('—')
    expect(html).toContain(
      'Honest placeholders: KPI values appear once the live funnel wiring exists — no fabricated numbers.',
    )
    expect(html).toContain('>—</p>')
    // A/B variants side by side with channel/variant tags.
    expect(html).toContain('Content calendar')
    expect(html).toContain(
      'Agent-drafted posts awaiting your decision. A/B variants render side by side; every approve or reject mints a receipt.',
    )
    expect(html).toContain('channel')
    expect(html).toContain('linkedin')
    expect(html).toContain('variant A')
    expect(html).toContain('variant B')
    expect(html).toContain('Post A')
    expect(html).toContain('Post B')
    // Draft items carry approve/reject buttons.
    expect(html).toContain('Approve')
    expect(html).toContain('Reject')
    // Decided item renders its receipt ref.
    expect(html).toContain('receipt: portal_content_decision:pcd_1')
  })

  test('KPI note switches off the honest-placeholder wording once a value is live', () => {
    const html = renderToStaticMarkup(
      <PortalSurface
        state={{
          ...READY_STATE,
          kpis: [
            { key: 'leads', label: 'Leads', value: 12, note: '' },
            { key: 'conversions', label: 'Conversions', value: null, note: '' },
          ],
        }}
      />,
    )
    expect(html).toContain('Live values where wired; placeholders stay explicit.')
    expect(html).not.toContain('Honest placeholders')
    expect(html).toContain('12')
    expect(html).toContain('—')
  })

  test('empty content calendar renders the honest no-items banner', () => {
    const html = renderToStaticMarkup(
      <PortalSurface state={{ ...READY_STATE, items: [] }} />,
    )
    expect(html).toContain(
      'No content items yet — drafts appear here as your team publishes the calendar.',
    )
    expect(html).not.toContain('Approve')
  })

  test('portalContentPairs groups A/B variants and keeps unpaired items alone', () => {
    const rows = portalContentPairs(READY_STATE.items)
    expect(rows).toHaveLength(2)
    expect(rows[0]!.map((entry) => entry.id)).toEqual(['item_a', 'item_b'])
    expect(rows[1]!.map((entry) => entry.id)).toEqual(['item_c'])
  })

  test('mount: logged-out fetch renders the login gate in real DOM, never engagement content', async () => {
    const mounted = await mount((async (input: RequestInfo | URL) => {
      const url = String(input)
      if (url.includes('/api/auth/session')) {
        return jsonResponse({ authenticated: false })
      }
      return jsonResponse({ error: 'not_found' }, 404)
    }) as typeof fetch)

    expect(mounted.textContent).toContain('Log in to view your engagement.')
    expect(
      mounted.querySelector('[data-portal-phase="logged_out"]'),
    ).not.toBeNull()
    for (const leak of ENGAGEMENT_CONTENT) {
      expect(mounted.textContent).not.toContain(leak)
    }
  })

  test('mount: logged-in with NO engagement renders the account email in real DOM (#8652 reopen)', async () => {
    const mounted = await mount((async (input: RequestInfo | URL) => {
      const url = String(input)
      if (url.includes('/api/auth/session')) {
        return jsonResponse({
          authenticated: true,
          bootstrap: {
            session: {
              userId: 'github:14167547',
              email: 'chris@openagents.com',
              login: 'AtlantisPleb',
            },
          },
        })
      }
      if (url.includes('/api/portal/engagement')) {
        return jsonResponse({ engagement: null })
      }
      return jsonResponse({ error: 'not_found' }, 404)
    }) as typeof fetch)

    expect(mounted.textContent).toContain('Your setup is being prepared')
    expect(mounted.textContent).toContain('Signed in as chris@openagents.com')
    expect(mounted.textContent).toContain('Sign out / switch account')
    expect(mounted.textContent).not.toContain('Content calendar')
  })

  test('mount: unreachable engagement API renders the honest unavailable state', async () => {
    const mounted = await mount((async (input: RequestInfo | URL) => {
      const url = String(input)
      if (url.includes('/api/auth/session')) {
        return jsonResponse({ authenticated: true })
      }
      return jsonResponse({ error: 'boom' }, 503)
    }) as typeof fetch)

    expect(mounted.textContent).toContain('Portal unavailable')
    expect(mounted.textContent).toContain(
      'The portal API is unreachable right now.',
    )
    expect(mounted.textContent).not.toContain('Content calendar')
  })

  test('mount: approve is optimistic and renders the minted receipt ref', async () => {
    let decisionCalls = 0
    let decisionMethod: string | undefined
    let decisionBody: unknown

    const mounted = await mount((async (
      input: RequestInfo | URL,
      init?: RequestInit,
    ) => {
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
        decisionMethod = init?.method
        decisionBody = JSON.parse(String(init?.body))
        return jsonResponse({
          ok: true,
          item: { state: 'approved' },
          receiptRef: 'portal_content_decision:pcd_live',
          alreadyDecided: false,
        })
      }
      return jsonResponse({ error: 'not_found' }, 404)
    }) as typeof fetch)

    expect(mounted.textContent).toContain('Strategic Consulting Demo')
    expect(mounted.textContent).toContain('Post A')

    const approve = Array.from(mounted.querySelectorAll('button')).find(
      button => button.textContent?.includes('Approve'),
    )
    expect(approve).toBeDefined()
    await act(async () => {
      approve!.click()
    })
    await flush()

    expect(decisionCalls).toBe(1)
    expect(decisionMethod).toBe('POST')
    expect(decisionBody).toEqual({ decision: 'approve' })
    expect(mounted.textContent).toContain(
      'receipt: portal_content_decision:pcd_live',
    )
    expect(mounted.textContent).toContain('approved')
    // Buttons are gone once decided.
    expect(
      Array.from(mounted.querySelectorAll('button')).some(button =>
        button.textContent?.includes('Approve'),
      ),
    ).toBe(false)
  })

  test('mount: failed decision rolls the item back to draft', async () => {
    const mounted = await mount((async (input: RequestInfo | URL) => {
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
    }) as typeof fetch)

    expect(mounted.textContent).toContain('Post A')
    const reject = Array.from(mounted.querySelectorAll('button')).find(
      button => button.textContent?.includes('Reject'),
    )
    expect(reject).toBeDefined()
    await act(async () => {
      reject!.click()
    })
    await flush()

    expect(mounted.textContent).toContain('Decision failed · refused')
    // Rolled back: the draft affordances are available again, and no receipt
    // was invented for a decision the server refused.
    expect(mounted.textContent).toContain('draft')
    expect(mounted.textContent).not.toContain('receipt:')
    expect(
      Array.from(mounted.querySelectorAll('button')).some(button =>
        button.textContent?.includes('Approve'),
      ),
    ).toBe(true)
  })

  test('source boundary: no Effect Native packages, and /portal keeps its route', () => {
    const read = (relativePath: string): string =>
      readFileSync(join(process.cwd(), relativePath), 'utf8')

    const core = read('src/routes/-portal-core.ts')
    const page = read('src/routes/-portal-page.tsx')
    const entry = read('src/portal-entry.ts')
    const routeSource = read('src/routes/portal.tsx')

    for (const source of [core, page, entry]) {
      expect(source).not.toContain('@effect-native')
    }
    // The state core stays host-free so the Cloud Run monolith entry can
    // bundle exactly the same logic the Start route runs.
    expect(core).not.toContain("from 'react")
    expect(core).toContain("from './-portal-data'")
    // The monolith entry mounts the same React surface, not a bespoke tree.
    expect(entry).toContain("from 'react-dom/client'")
    expect(entry).toContain("from './routes/-portal-page'")
    expect(routeSource).toContain("createFileRoute('/portal')")
  })
})
