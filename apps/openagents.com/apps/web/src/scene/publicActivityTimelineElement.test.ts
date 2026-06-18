import {
  orderPublicActivityTimelineEvents,
  type PublicActivityTimelineEnvelope,
} from '@openagentsinc/public-activity-timeline'
import {
  activeTimelineFixture,
  realBitcoinTimelineFixture,
  staleTimelineFixture,
} from '@openagentsinc/public-activity-timeline/fixtures'
import { afterEach, describe, expect, test, vi } from 'vitest'

import {
  PUBLIC_ACTIVITY_TIMELINE_ENDPOINT,
  PUBLIC_ACTIVITY_TIMELINE_TAG,
  mountPublicActivityTimeline,
  publicActivityTimelineView,
} from './publicActivityTimelineElement'

const jsonResponse = (body: unknown, status = 200): Response =>
  ({
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  }) as unknown as Response

const dashboardEnvelope = (): PublicActivityTimelineEnvelope => ({
  ...activeTimelineFixture,
  generatedAt: '2026-06-18T19:00:00.000Z',
  nextCursor: realBitcoinTimelineFixture.nextCursor,
  sourceLag: [
    ...activeTimelineFixture.sourceLag,
    ...staleTimelineFixture.sourceLag,
    {
      sourceKind: 'settlement_receipt',
      status: 'current',
      latestSourceEventAt: '2026-06-18T18:04:02.000Z',
      observedAt: '2026-06-18T19:00:00.000Z',
      lagSeconds: 0,
      maxStalenessSeconds: 0,
      sourceRefs: ['receipt.public.real.1'],
      blockerRefs: [],
      caveatRefs: [],
    },
  ],
  events: orderPublicActivityTimelineEvents([
    ...activeTimelineFixture.events,
    ...realBitcoinTimelineFixture.events,
    ...staleTimelineFixture.events,
  ]),
})

const waitForState = async (
  root: HTMLElement,
  state: string,
): Promise<void> => {
  for (let index = 0; index < 50 && root.dataset.state !== state; index += 1) {
    await Promise.resolve()
    await new Promise(resolve => setTimeout(resolve, 0))
  }
}

const mountWithPayload = async (
  payload: unknown,
): Promise<{
  fetchMock: ReturnType<typeof vi.fn>
  handle: ReturnType<typeof mountPublicActivityTimeline>
  root: HTMLElement
}> => {
  const root = document.createElement('div')
  document.body.append(root)
  const fetchMock = vi.fn(async () => jsonResponse(payload))
  const handle = mountPublicActivityTimeline(root, {
    fetchFn: fetchMock as unknown as typeof fetch,
    refreshIntervalMs: 0,
  })
  return { fetchMock, handle, root }
}

describe('public activity timeline element', () => {
  afterEach(() => {
    document.body.replaceChildren()
    vi.restoreAllMocks()
  })

  test('renders Fleet, Money Loop, Forum, Timeline, and Proof Drawer panes', async () => {
    const { fetchMock, handle, root } = await mountWithPayload(
      dashboardEnvelope(),
    )

    await waitForState(root, 'ok')

    expect(fetchMock).toHaveBeenCalledWith(PUBLIC_ACTIVITY_TIMELINE_ENDPOINT, {
      cache: 'no-store',
      headers: { accept: 'application/json' },
    })
    expect(root.querySelector('[data-activity-pane="fleet"]')?.textContent).toContain(
      'Pylon heartbeat observed.',
    )
    expect(root.querySelector('[data-activity-pane="money"]')?.textContent).toContain(
      'Receipt-backed real Bitcoin movement confirmed.',
    )
    expect(root.querySelector('[data-activity-pane="forum"]')?.textContent).toContain(
      'Public Forum topic created.',
    )
    expect(root.querySelector('[data-activity-pane="timeline"]')).not.toBeNull()
    expect(root.querySelector('[data-proof-drawer]')).not.toBeNull()
    expect(root.querySelector('[data-source-status="stale"]')?.textContent).toContain(
      'forum',
    )

    handle.dispose()
  })

  test('selects an event and renders reproducible public proof details', async () => {
    const { handle, root } = await mountWithPayload(dashboardEnvelope())

    await waitForState(root, 'ok')
    const realBitcoinButton = Array.from(
      root.querySelectorAll<HTMLButtonElement>('[data-activity-event]'),
    ).find(button =>
      button.textContent?.includes('real Bitcoin movement confirmed'),
    )
    expect(realBitcoinButton).toBeDefined()

    realBitcoinButton?.click()

    const proof = root.querySelector('[data-proof-drawer]')
    expect(proof?.textContent).toContain('receipt.public.real.1')
    expect(proof?.textContent).toContain('settlement receipt')
    expect(proof?.querySelector('[data-proof-source-api]')?.textContent).toBe(
      PUBLIC_ACTIVITY_TIMELINE_ENDPOINT,
    )
    expect(proof?.querySelector('[data-proof-event-json]')?.textContent).toContain(
      '"realBitcoinMoved": true',
    )

    handle.dispose()
  })

  test('does not render private material from unsafe raw payload fields', async () => {
    const { handle, root } = await mountWithPayload({
      ...dashboardEnvelope(),
      rawPrivateTrace:
        '/Users/christopherdavid/private raw provider payload bearer token',
    })

    await waitForState(root, 'error')

    expect(root.textContent).toContain('Timeline unavailable')
    expect(root.textContent).not.toContain('/Users/christopherdavid')
    expect(root.textContent).not.toContain('bearer token')

    handle.dispose()
  })

  test('smokes the browser element shell and text-fit CSS constraints', () => {
    vi.spyOn(globalThis, 'fetch').mockImplementation(
      () => new Promise<Response>(() => undefined),
    )
    publicActivityTimelineView()
    const element = document.createElement(PUBLIC_ACTIVITY_TIMELINE_TAG)
    document.body.append(element)

    const styleText = element.shadowRoot?.querySelector('style')?.textContent ?? ''

    expect(element.shadowRoot?.querySelector('div')).not.toBeNull()
    expect(styleText).toContain('overflow-wrap: anywhere')
    expect(styleText).toContain('@media (max-width: 640px)')
    expect(styleText).toContain('.timeline-button:hover')
  })
})
