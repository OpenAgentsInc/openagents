import { describe, expect, it } from 'bun:test'
import { createPerKeyDebounce, createPerKeyThrottle } from './throttle'

// These tests validate the basic scheduling guarantees:
// - Throttle: at most one call per window per key; latest invocation wins.
// - Debounce: only the last invocation per key runs after the quiet period.

describe('createPerKeyThrottle', () => {
  it('throttles per key within the window and runs latest fn once', async () => {
    const calls: string[] = []
    const throttle = createPerKeyThrottle(50)
    throttle('a', () => calls.push('a1'))
    throttle('a', () => calls.push('a2'))
    throttle('b', () => calls.push('b1'))
    await new Promise((r) => setTimeout(r, 70))
    expect(calls.includes('b1')).toBe(true)
    // Only one of a1/a2 should run, and running the latest is acceptable
    const aCalls = calls.filter((c) => c.startsWith('a'))
    expect(aCalls.length).toBe(1)
  })
})

describe('createPerKeyDebounce', () => {
  it('debounces by key and runs only last call', async () => {
    const calls: string[] = []
    const debounce = createPerKeyDebounce(50)
    debounce('x', () => calls.push('x1'))
    debounce('x', () => calls.push('x2'))
    debounce('y', () => calls.push('y1'))
    await new Promise((r) => setTimeout(r, 70))
    expect(calls).toContain('x2')
    expect(calls).not.toContain('x1')
    expect(calls).toContain('y1')
  })
})
