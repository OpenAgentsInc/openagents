import { describe, expect, test, vi } from 'vitest'

import { mountPylonStats } from './pylonStatsElement'
import type { PylonStatsSnapshot } from './pylonNetworkStats'

const statKeys = ['online', 'working', 'sats24h', 'training'] as const

const makeRoot = (): HTMLElement => {
  const root = document.createElement('div')
  for (const key of statKeys) {
    const value = document.createElement('div')
    value.setAttribute('data-stat-value', key)
    root.append(value)
  }
  return root
}

const pendingFetch = () =>
  vi.fn(
    () => new Promise<Response>(() => undefined),
  ) as unknown as typeof fetch

const statText = (root: HTMLElement, key: (typeof statKeys)[number]): string =>
  Array.from(
    root.querySelectorAll(`[data-stat-value="${key}"] .char-sizer`),
  )
    .map(node => node.textContent ?? '')
    .join('') ||
  root.querySelector(`[data-stat-value="${key}"]`)?.textContent ||
  ''

describe('mountPylonStats boot snapshot', () => {
  test('seeds first paint from the embedded public stats snapshot', () => {
    const root = makeRoot()
    const snapshot: PylonStatsSnapshot = {
      available: true,
      publicRealSatsSettled24h: 150_000,
      pylonSessionsOnlineNow: 9,
      pylonsAssignmentReadyNow: 2,
      pylonsOnlineNow: 4,
      trainingModelProgressContributors: 3,
    }

    const handle = mountPylonStats(root, {
      fetchFn: pendingFetch(),
      initialSnapshot: snapshot,
      intervalMs: 60_000,
    })

    expect(statText(root, 'online')).toContain('4')
    expect(statText(root, 'working')).toContain('2')
    expect(statText(root, 'sats24h')).toContain('150,000')
    expect(statText(root, 'training')).toContain('3')
    expect(root.textContent).not.toContain('…')

    handle.dispose()
  })

  test('keeps the loading placeholder when no embedded snapshot exists', () => {
    const root = makeRoot()
    const handle = mountPylonStats(root, {
      fetchFn: pendingFetch(),
      intervalMs: 60_000,
    })

    expect(statText(root, 'online')).toContain('…')

    handle.dispose()
  })
})
