import { act } from 'react'
import { type Root, createRoot } from 'react-dom/client'
import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, describe, expect, test } from 'vitest'

import {
  type QaBoardProjection,
  fetchQaBoard,
  freshnessLabel,
} from './-qa-board-data'
import { QaBoardPage } from './-qa-board-page'

;(
  globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true

const projection: QaBoardProjection = {
  schema: 'openagents.qa.board.v1',
  servedAt: '2026-07-16T16:00:00.000Z',
  sources: { issues: 'ok', observer: 'ok', swarm: 'ok' },
  observer: {
    runAt: '2026-07-16T15:58:00.000Z',
    checks: [
      {
        consecutiveDriftRuns: 0,
        durationMs: 42,
        id: 'public.product_promises',
        severityOnDrift: 'high',
        status: 'drift',
        surface: 'https://openagents.com/api/public/product-promises',
      },
      {
        consecutiveDriftRuns: 0,
        durationMs: 18,
        id: 'public.pylon_stats',
        severityOnDrift: 'medium',
        status: 'pass',
        surface: 'https://openagents.com/api/public/pylon-stats',
      },
    ],
    summary: { drift: 1, pass: 1, total: 2, unrunnable: 0 },
  },
  swarm: {
    baseSha: '46ea6c28f04b0fc29df482227a2c8e87b16968c5',
    completedAt: '2026-07-16T15:59:00.000Z',
    lanes: [
      {
        id: 'web_routes',
        surface: 'openagents.com public web routes',
        verdict: 'pass',
      },
      {
        id: 'payments_promises',
        surface: 'product promises',
        verdict: 'finding',
      },
    ],
    runRef: 'qa.six-lane.20260716T155900Z',
    verdict: 'findings',
  },
  findings: [
    {
      issueNumber: 8912,
      issueState: 'open',
      issueUrl: 'https://github.com/OpenAgentsInc/openagents/issues/8912',
      severity: 'high',
      summary:
        'Confirmed high-severity finding on public product-promise registry.',
      surface: 'public product-promise registry',
    },
  ],
}

const response = (body: unknown, status = 200): Response =>
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

const mount = async (fetchFn: typeof fetch): Promise<HTMLDivElement> => {
  globalThis.fetch = fetchFn
  container = document.createElement('div')
  document.body.appendChild(container)
  root = createRoot(container)
  await act(async () => root?.render(<QaBoardPage />))
  await act(async () => {})
  await act(async () => {})
  return container
}

describe('Start /qa live board', () => {
  test('server-renders a truthful loading shell', () => {
    const html = renderToStaticMarkup(<QaBoardPage />)
    expect(html).toContain('data-route="qa"')
    expect(html).toContain('Live QA board')
    expect(html).toContain('Loading QA evidence')
    expect(html).toContain('Refreshing evidence…')
    expect(html).not.toContain('0 open')
  })

  test('renders live observer, swarm, finding, freshness, and source states', async () => {
    const mounted = await mount((async () =>
      response(projection)) as typeof fetch)
    expect(mounted.querySelector('[data-qa-state="ok"]')).not.toBeNull()
    expect(mounted.textContent).toContain('1/2 passing')
    expect(mounted.textContent).toContain('public.product_promises')
    expect(mounted.textContent).toContain('payments_promises')
    expect(mounted.textContent).toContain('public product-promise registry')
    expect(mounted.textContent).toContain('#8912 · open')
    expect(mounted.textContent).toContain('GitHub issue ledger')
    expect(
      mounted.querySelector(
        'a[href="https://github.com/OpenAgentsInc/openagents/issues/8912"]',
      ),
    ).not.toBeNull()
  })

  test('renders the endpoint failure detail instead of fake-green content', async () => {
    const mounted = await mount((async () =>
      response({ error: 'down' }, 503)) as typeof fetch)
    const degraded = mounted.querySelector('[data-qa-state="unavailable"]')
    expect(degraded).not.toBeNull()
    expect(degraded?.textContent).toContain('Unavailable')
    expect(degraded?.textContent).toContain('HTTP 503')
    expect(mounted.textContent).not.toContain('0/0 passing')
  })

  test('renders the bounded empty-findings explanation', async () => {
    const mounted = await mount((async () =>
      response({
        ...projection,
        findings: [],
        sources: { ...projection.sources, issues: 'empty' },
      })) as typeof fetch)
    expect(mounted.querySelector('[data-qa-empty="findings"]')).not.toBeNull()
    expect(mounted.textContent).toContain('No open issue-linked findings.')
    expect(mounted.textContent).toContain(
      'does not claim that unobserved surfaces are healthy',
    )
  })
})

describe('QA board data boundary', () => {
  test('accepts only the QA board projection schema', async () => {
    await expect(
      fetchQaBoard((async () => response(projection)) as typeof fetch),
    ).resolves.toEqual({
      state: 'ok',
      data: projection,
    })
    await expect(
      fetchQaBoard((async () => response({ schema: 'other' })) as typeof fetch),
    ).resolves.toMatchObject({
      state: 'unavailable',
      detail: expect.stringContaining('unsupported'),
    })
  })

  test('formats bounded freshness without future-negative ages', () => {
    const now = Date.parse('2026-07-16T16:00:00.000Z')
    expect(freshnessLabel('2026-07-16T15:32:00.000Z', now)).toBe(
      'Updated 28m ago',
    )
    expect(freshnessLabel('2026-07-18T15:32:00.000Z', now)).toBe(
      'Updated less than a minute ago',
    )
    expect(freshnessLabel('not-a-date', now)).toBe('Freshness unavailable')
  })
})
