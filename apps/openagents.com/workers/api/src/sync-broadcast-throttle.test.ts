import { describe, expect, test } from 'vitest'

import {
  HIGH_FREQUENCY_BROADCAST_MIN_INTERVAL_MS,
  decideHighFrequencyBroadcast,
  highFrequencyBroadcastLastAtStorageKey,
  isThrottledBroadcastScope,
} from './sync-broadcast-throttle'

const THROTTLED_SCOPE = 'public-khala-tokens-served:network'

// A tiny in-memory stand-in for the DO durable storage used by the throttle.
// Crucially, the throttle reads `lastBroadcastAt` from this DURABLE map on every
// poke and persists back to it, so it survives a simulated hibernation (we never
// reset this map across the burst). This is exactly the state the old in-memory
// `lastBroadcastAtMs` Map + alarm-pending Set lost on hibernation (#6324).
const makeDurableThrottleStore = () => {
  const store = new Map<string, number>()
  // Drive the throttle the way the DO does: read durable last-at, decide, persist.
  const poke = (scope: string, nowMs: number): boolean => {
    const key = highFrequencyBroadcastLastAtStorageKey(scope)
    const decision = decideHighFrequencyBroadcast({
      scope,
      nowMs,
      lastBroadcastAtMs: store.get(key) ?? null,
    })
    if (decision.broadcast && decision.persistLastBroadcastAtMs !== undefined) {
      store.set(key, decision.persistLastBroadcastAtMs)
    }
    return decision.broadcast
  }
  return { store, poke }
}

describe('high-frequency broadcast throttle (#6324)', () => {
  test('throttled scope predicate matches only the khala tokens-served scope', () => {
    expect(isThrottledBroadcastScope(THROTTLED_SCOPE)).toBe(true)
    expect(isThrottledBroadcastScope('public-khala-tokens-served:other')).toBe(
      true,
    )
    expect(isThrottledBroadcastScope('team:42')).toBe(false)
    expect(isThrottledBroadcastScope('tassadar-settled-feed:network')).toBe(
      false,
    )
  })

  test('first poke always broadcasts (leading edge), even with no durable state', () => {
    const decision = decideHighFrequencyBroadcast({
      scope: THROTTLED_SCOPE,
      nowMs: 1_000,
      lastBroadcastAtMs: null,
    })
    expect(decision.broadcast).toBe(true)
    expect(decision.persistLastBroadcastAtMs).toBe(1_000)
  })

  test('non-throttled scopes always broadcast immediately and persist nothing', () => {
    const decision = decideHighFrequencyBroadcast({
      scope: 'team:7',
      nowMs: 5,
      lastBroadcastAtMs: 4,
    })
    expect(decision.broadcast).toBe(true)
    expect(decision.persistLastBroadcastAtMs).toBeUndefined()
  })

  test('a poke inside the window is skipped and does not advance durable state', () => {
    const inside = decideHighFrequencyBroadcast({
      scope: THROTTLED_SCOPE,
      nowMs: 1_100,
      lastBroadcastAtMs: 1_000,
    })
    expect(inside.broadcast).toBe(false)
    expect(inside.persistLastBroadcastAtMs).toBeUndefined()
  })

  test('a poke exactly on the slot boundary fires (>= boundary, never stalls)', () => {
    const onBoundary = decideHighFrequencyBroadcast({
      scope: THROTTLED_SCOPE,
      nowMs: 1_000 + HIGH_FREQUENCY_BROADCAST_MIN_INTERVAL_MS,
      lastBroadcastAtMs: 1_000,
    })
    expect(onBoundary.broadcast).toBe(true)
    expect(onBoundary.persistLastBroadcastAtMs).toBe(
      1_000 + HIGH_FREQUENCY_BROADCAST_MIN_INTERVAL_MS,
    )
  })

  test('sustained ~42/sec burst broadcasts at a steady ≤3/sec with NO freeze, across hibernation', () => {
    const { poke } = makeDurableThrottleStore()

    // Simulate the GLM surge: ~42 pokes/sec for 5 seconds = 210 pokes, evenly
    // spaced. We model HIBERNATION by never carrying any in-memory state between
    // pokes — every decision is taken solely from the durable store inside `poke`.
    const POKES_PER_SEC = 42
    const DURATION_SEC = 5
    const totalPokes = POKES_PER_SEC * DURATION_SEC
    const gapMs = 1_000 / POKES_PER_SEC

    const broadcastTimes: Array<number> = []
    for (let i = 0; i < totalPokes; i++) {
      const nowMs = Math.round(i * gapMs)
      if (poke(THROTTLED_SCOPE, nowMs)) {
        broadcastTimes.push(nowMs)
      }
    }

    // The counter NEVER freezes: there is at least one broadcast in every
    // 1-second window of the burst (the old bug dropped all trailing broadcasts
    // during the burst, freezing the counter until it subsided).
    for (let windowStart = 0; windowStart < DURATION_SEC * 1_000; windowStart += 1_000) {
      const inWindow = broadcastTimes.filter(
        t => t >= windowStart && t < windowStart + 1_000,
      )
      expect(inWindow.length).toBeGreaterThanOrEqual(1)
      // ≤3/sec cap holds: with a 334ms min interval, at most 3 fire per second.
      expect(inWindow.length).toBeLessThanOrEqual(3)
    }

    // Every consecutive pair of broadcasts is spaced at least the min interval.
    for (let i = 1; i < broadcastTimes.length; i++) {
      const current = broadcastTimes[i] ?? 0
      const previous = broadcastTimes[i - 1] ?? 0
      expect(current - previous).toBeGreaterThanOrEqual(
        HIGH_FREQUENCY_BROADCAST_MIN_INTERVAL_MS,
      )
    }

    // Sanity: over a 5s burst at 334ms spacing we get ~14-15 broadcasts, not the
    // single post-burst jump the old alarm path collapsed to.
    expect(broadcastTimes.length).toBeGreaterThanOrEqual(13)
    expect(broadcastTimes.length).toBeLessThanOrEqual(16)
  })

  test('the durable-state path keeps firing across an explicit hibernation boundary', () => {
    const { store, poke } = makeDurableThrottleStore()

    // Poke once, then simulate the DO hibernating: the OLD code kept its pending
    // trailing scope + last-at in instance memory, which would be wiped here. We
    // model that wipe as "any non-durable cache is gone" — but the durable store
    // persists, so the throttle still has lastBroadcastAt and keeps cadence.
    expect(poke(THROTTLED_SCOPE, 0)).toBe(true)
    expect(store.size).toBe(1)

    // (hibernation happens here — no in-memory state survives, only `store`)

    // A poke just inside the window after waking is correctly skipped using the
    // durable last-at (not treated as a fresh leading edge, which would let the
    // burst exceed the cap).
    expect(poke(THROTTLED_SCOPE, 100)).toBe(false)
    // A poke past the window after waking fires — the counter advances post-wake
    // instead of freezing.
    expect(poke(THROTTLED_SCOPE, 400)).toBe(true)
  })

  test('a single quiet poke after a long idle always advances (final post-burst delta)', () => {
    const { poke } = makeDurableThrottleStore()
    expect(poke(THROTTLED_SCOPE, 1_000)).toBe(true)
    // Long idle, then one poke far later: must fire so the last delta lands.
    expect(poke(THROTTLED_SCOPE, 60_000)).toBe(true)
  })
})
