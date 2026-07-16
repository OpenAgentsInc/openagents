import { act } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, beforeEach, describe, expect, test } from 'vitest'

import {
  accountingPanelValues,
  fetchPublicJson,
  FORUM_LAUNCH_STATUS_URL,
  forumPanelValues,
  historyBars,
  mixRows,
  nostrPanelValues,
  pylonPanelValues,
  STATS_PYLON_STATS_URL,
  tokensServedDisplay,
  TOKENS_SERVED_CHANNEL_MIX_URL,
  TOKENS_SERVED_HISTORY_URL,
  TOKENS_SERVED_MODEL_MIX_URL,
  TOKENS_SERVED_URL,
} from './-stats-data'
import { StatsPage } from './-stats-page'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const livePayloads: Record<string, unknown> = {
  [FORUM_LAUNCH_STATUS_URL]: {
    gates: [{ id: 'listed_forum_agent_posting', label: 'Listed', state: 'ready' }],
    orangeChecksSold: 2,
    publicTipping: {
      gates: [{ id: 'tip_recipient_readiness', label: 'Tips', state: 'ready' }],
    },
    status: 'ready',
    summary: 'Forum posting is ready for broader public launch.',
  },
  [STATS_PYLON_STATS_URL]: {
    asOfLabel: 'Just now',
    available: true,
    earningLaunchGate: {
      state: 'ready',
      stateLabel: 'Ready for bounded public earning copy',
    },
    hostedNexusRelayUrl: null,
    nexusAcceptedWorkPayoutSatsPaidTotal: 844,
    nexusAcceptedWorkSettlementGate: {
      settledReceiptRefs: ['receipt.a', 'receipt.b'],
      state: 'ready',
      stateLabel: 'Receipt-backed accepted-work settlement totals ready',
    },
    pylonsAssignmentReadyNow: 2,
    pylonsOnlineNow: 2,
    pylonsSeen24h: 6,
    recentPylons: [
      { nostrPubkeyShort: 'pylon.abc', relayUrls: [] },
      { nostrPubkeyShort: 'pylon.def', relayUrls: ['wss://relay.example'] },
    ],
    status: 'live',
  },
  [TOKENS_SERVED_CHANNEL_MIX_URL]: {
    groups: [
      { channel: 'khala_api', label: 'Khala API', pct: 96.42, reqs: 449937, tokens: 8160667542 },
      { channel: 'direct_local', label: 'Direct local', pct: 3.58, reqs: 70, tokens: 302571194 },
    ],
    totalTokens: 8463238736,
    window: '30d',
  },
  [TOKENS_SERVED_HISTORY_URL]: {
    series: [
      { day: '2026-07-14', tokensServed: 2584580 },
      { day: '2026-07-15', tokensServed: 0 },
      { day: '2026-07-16', tokensServed: 1000 },
    ],
    timezone: 'America/Chicago',
    window: '30d',
  },
  [TOKENS_SERVED_MODEL_MIX_URL]: {
    groups: [
      { family: 'pylon_codex', label: 'Pylon-Codex', pct: 89.56, reqs: 2684, tokens: 7580053905 },
      { family: 'gemini', label: 'Gemini', pct: 1.27854, reqs: 66891, tokens: 108205925 },
    ],
    totalTokens: 8463238736,
    window: '30d',
  },
  [TOKENS_SERVED_URL]: {
    generatedAt: '2026-07-16T12:40:31.483Z',
    schemaVersion: 'openagents.public_khala_tokens_served.v1',
    tokensServed: 8463301501,
  },
}

const jsonResponse = (body: unknown): Response =>
  ({ json: async () => body, ok: true, status: 200 }) as unknown as Response

const stubFetchWith = (handler: (url: string) => Promise<Response>): void => {
  globalThis.fetch = ((input: RequestInfo | URL) =>
    handler(String(input))) as unknown as typeof fetch
}

const originalFetch = globalThis.fetch

let container: HTMLDivElement | null = null
let root: Root | null = null

const mountStatsPage = async (): Promise<HTMLDivElement> => {
  container = document.createElement('div')
  document.body.appendChild(container)
  root = createRoot(container)
  await act(async () => {
    root?.render(<StatsPage />)
  })
  // Flush the fetch -> json -> setState microtask chain.
  await act(async () => {})
  await act(async () => {})
  return container
}

beforeEach(() => {
  container = null
  root = null
})

afterEach(async () => {
  if (root !== null) {
    await act(async () => {
      root?.unmount()
    })
  }
  container?.remove()
  globalThis.fetch = originalFetch
})

describe('Start /stats route (public/anonymous variant)', () => {
  test('server-renders the route contract and hero copy', () => {
    const html = renderToStaticMarkup(<StatsPage />)

    expect(html).toContain('data-route="stats"')
    expect(html).toContain('Network Stats')
    expect(html).toContain(
      'Live public-safe evidence: receipt-backed counters, launch gates, and claim boundaries. No dummy values; missing evidence is marked unavailable.',
    )
  })

  test('server render keeps the honest pre-fetch idle placeholders (no fabricated values)', () => {
    const html = renderToStaticMarkup(<StatsPage />)

    expect(html).toContain('data-counter="khala-tokens-served"')
    expect(html).toContain('data-counter-display="khala-tokens-served"')
    expect(html).toContain('data-value="—"')
    expect(html).toContain('data-status="loading"')
    expect(html).toContain('Tokens Served')
    expect(html).toContain('data-chart="khala-tokens-served-history"')
    expect(html).toContain('Waiting for data…')
    expect(html).toContain('Waiting for model mix…')
    expect(html).toContain('Waiting for channel mix…')
    expect((html.match(/Unavailable/g) ?? []).length).toBeGreaterThan(5)
  })

  test('preserves the Claim Boundary and Endpoint Manifest panels', () => {
    const html = renderToStaticMarkup(<StatsPage />)

    expect(html).toContain('Claim Boundary')
    expect(html).toContain('Public copy boundaries for money and earning claims.')
    expect(html).toContain('Bounded')
    expect(html).toContain('Endpoint Manifest')
    expect(html).toContain('/.well-known/openagents.json')
    expect(html).toContain('/api/openapi.json')
    expect(html).toContain('/api/public/pylon-stats')
    expect(html).toContain('/api/public/adjutant/activity')
  })

  test('names the live-fetched endpoints and the honest tip/revshare boundary', () => {
    const html = renderToStaticMarkup(<StatsPage />)

    expect(html).toContain('/api/public/khala-tokens-served')
    expect(html).toContain('/api/public/khala-tokens-served/history')
    expect(html).toContain('/api/forum/launch-status')
    expect(html).toContain('This route fetches live public data on the client')
    expect(html).toContain('money_surface_retired')
    // The retired tip endpoint is no longer advertised as a live source.
    expect(html).not.toContain('/api/forum/tip-leaderboards')
  })

  test('preserves the Nostr Relay Configuration panel and the home link', () => {
    const html = renderToStaticMarkup(<StatsPage />)

    expect(html).toContain('Nostr Relay Configuration')
    expect(html).toContain('No relay endpoint list is public in the current response.')
    expect(html).toContain('href="/"')
  })

  test('renders the fetched tokens-served counter and panel values after the client fetch resolves', async () => {
    stubFetchWith(async url => {
      const body = livePayloads[url]
      if (body === undefined) throw new Error(`unexpected fetch: ${url}`)
      return jsonResponse(body)
    })

    const mounted = await mountStatsPage()

    const counter = mounted.querySelector('[data-counter-display="khala-tokens-served"]')
    expect(counter?.getAttribute('data-status')).toBe('ok')
    expect(counter?.getAttribute('data-value')).toBe('8,463,301,501')
    expect(counter?.textContent).toBe('8,463,301,501')

    // History chart renders one bar per day from the fetched series.
    expect(
      mounted.querySelectorAll('[data-chart="khala-tokens-served-history"] [data-history-day]')
        .length,
    ).toBe(3)
    // Mix panels render the fetched groups, not placeholders.
    expect(mounted.textContent).toContain('Pylon-Codex')
    expect(mounted.textContent).toContain('Khala API')
    expect(mounted.textContent).toContain('Direct local')
    expect(mounted.textContent).not.toContain('Waiting for data…')

    // Pylon panel carries the live projection values.
    const pylonPanel = mounted.querySelector('[data-stats-pylon-panel]')
    expect(pylonPanel?.getAttribute('data-stats-pylon-panel')).toBe('ok')
    expect(pylonPanel?.textContent).toContain('Heartbeat freshness: Just now.')
    expect(pylonPanel?.textContent).toContain('Ready for bounded public earning copy')

    // Accounting strip carries receipt-backed values; revshare stays honest.
    const accountingPanel = mounted.querySelector('[data-stats-accounting-panel]')
    expect(accountingPanel?.textContent).toContain('844 sats')
    expect(accountingPanel?.textContent).toContain('2 receipts')
    expect(accountingPanel?.textContent).toContain('Unavailable')

    // Forum panel: launch gates are live, retired tip totals stay Unavailable.
    const forumPanel = mounted.querySelector('[data-stats-forum-panel]')
    expect(forumPanel?.getAttribute('data-stats-forum-panel')).toBe('ok')
    expect(forumPanel?.textContent).toContain('Ready')
    expect(forumPanel?.textContent).toContain('Unavailable')
  })

  test('a failed fetch degrades to the honest unavailable state, never a dummy number', async () => {
    stubFetchWith(async () => {
      throw new Error('offline in test')
    })

    const mounted = await mountStatsPage()

    const counter = mounted.querySelector('[data-counter-display="khala-tokens-served"]')
    expect(counter?.getAttribute('data-status')).toBe('unavailable')
    expect(counter?.getAttribute('data-value')).toBe('—')
    expect(counter?.textContent).toBe('—')

    expect(mounted.textContent).toContain('History unavailable.')
    expect(mounted.textContent).toContain('Model mix unavailable.')
    expect(mounted.textContent).toContain('Channel mix unavailable.')
    expect(mounted.querySelector('[data-stats-pylon-panel]')?.getAttribute('data-stats-pylon-panel')).toBe(
      'unavailable',
    )
    expect(mounted.textContent).toContain('Heartbeat freshness unavailable.')
    expect((mounted.textContent?.match(/Unavailable/g) ?? []).length).toBeGreaterThan(8)
  })

  test('a non-OK response is treated as unavailable by the fail-soft fetcher', async () => {
    const gone = { json: async () => ({ error: 'money_surface_retired' }), ok: false, status: 410 }
    const result = await fetchPublicJson(
      '/api/forum/tip-leaderboards',
      (async () => gone as unknown as Response) as unknown as typeof fetch,
    )
    expect(result).toBeNull()
  })

  test('tokensServedDisplay never fabricates a number', () => {
    expect(tokensServedDisplay({ state: 'loading' })).toEqual({ live: false, value: '—' })
    expect(tokensServedDisplay({ state: 'unavailable' })).toEqual({ live: false, value: '—' })
    expect(tokensServedDisplay({ data: {}, state: 'ok' })).toEqual({ live: false, value: '—' })
    expect(
      tokensServedDisplay({ data: { tokensServed: 8463301501 }, state: 'ok' }),
    ).toEqual({ live: true, value: '8,463,301,501' })
  })

  test('historyBars supports daily and cumulative metrics without inventing rows', () => {
    const snapshot = {
      series: [
        { day: '2026-07-14', tokensServed: 100 },
        { day: '2026-07-15', tokensServed: 0 },
        { day: '2026-07-16', tokensServed: 300 },
        { day: 'bad-row' },
      ],
    }
    const daily = historyBars(snapshot, 'daily')
    expect(daily.map(bar => bar.tokens)).toEqual([100, 0, 300])
    expect(daily[2]?.heightPct).toBe(100)
    expect(daily[1]?.heightPct).toBe(0)

    const cumulative = historyBars(snapshot, 'cumulative')
    expect(cumulative.map(bar => bar.tokens)).toEqual([100, 100, 400])
    expect(cumulative[2]?.heightPct).toBe(100)

    expect(historyBars({}, 'daily')).toEqual([])
  })

  test('mix, pylon, accounting, nostr, and forum derivations stay fail-soft', () => {
    expect(mixRows({})).toEqual([])
    expect(
      mixRows({ groups: [{ label: 'Gemini', pct: 1.27854, reqs: 66891, tokens: 108205925 }] }),
    ).toEqual([
      { detail: '108,205,925 tokens · 66,891 reqs', label: 'Gemini', pct: '1.28%' },
    ])

    expect(pylonPanelValues({ available: false })).toBeNull()
    expect(pylonPanelValues({ status: 'unavailable' })).toBeNull()
    expect(pylonPanelValues({})?.onlineNow).toBe('Unavailable')

    expect(accountingPanelValues({ available: false })).toBeNull()
    expect(accountingPanelValues({})?.acceptedWorkSatsPaid).toBe('Unavailable')

    expect(nostrPanelValues({ available: false })).toBeNull()
    expect(nostrPanelValues({})).toBeNull()
    expect(
      nostrPanelValues({
        recentPylons: [{ nostrPubkeyShort: 'pk.a', relayUrls: ['wss://r'] }],
      }),
    ).toEqual({ pubkeys: '1 recent', relayUrls: '1 published' })

    expect(forumPanelValues({})?.tipGate).toBe('Unavailable')
    expect(
      forumPanelValues({
        publicTipping: { gates: [{ id: 'g', state: 'ready' }] },
        summary: 'ready',
      }),
    ).toMatchObject({ tipGate: 'Ready', tipGateReady: true })
  })
})
