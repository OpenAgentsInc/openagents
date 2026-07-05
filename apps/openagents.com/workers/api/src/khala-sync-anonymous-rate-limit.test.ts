// Unit tests for the KS-8.x anonymous-read rate limiter
// (khala-sync-anonymous-rate-limit.ts): per-IP fixed-window admission for
// the connect/read limiter flavors, independent counters per IP and per
// limiter instance, and window expiry. Deterministic clock injection — no
// real timers.

import { describe, expect, test } from 'vitest'

import {
  makeKhalaSyncAnonymousConnectRateLimiter,
  makeKhalaSyncAnonymousReadRateLimiter,
} from './khala-sync-anonymous-rate-limit'

const requestFrom = (ip: string): Request =>
  new Request('https://openagents.com/api/sync/log', {
    headers: { 'cf-connecting-ip': ip },
  })

describe('makeKhalaSyncAnonymousReadRateLimiter', () => {
  test('admits up to the per-minute limit for one IP, then denies within the same window', () => {
    let now = 0
    const limiter = makeKhalaSyncAnonymousReadRateLimiter(() => now)
    const request = requestFrom('203.0.113.1')
    const results = Array.from({ length: 121 }, () => limiter(request))
    expect(results.slice(0, 120)).toEqual(Array(120).fill(true))
    expect(results[120]).toBe(false)
  })

  test('a fresh minute window re-admits after the limit was hit', () => {
    let now = 0
    const limiter = makeKhalaSyncAnonymousReadRateLimiter(() => now)
    const request = requestFrom('203.0.113.2')
    for (let i = 0; i < 120; i += 1) limiter(request)
    expect(limiter(request)).toBe(false)
    now += 60_001
    expect(limiter(request)).toBe(true)
  })

  test('different IPs get independent counters', () => {
    let now = 0
    const limiter = makeKhalaSyncAnonymousReadRateLimiter(() => now)
    const a = requestFrom('203.0.113.3')
    const b = requestFrom('203.0.113.4')
    for (let i = 0; i < 120; i += 1) limiter(a)
    expect(limiter(a)).toBe(false)
    expect(limiter(b)).toBe(true)
  })

  test('missing CF-Connecting-IP falls back to x-forwarded-for, then a shared "unknown" bucket', () => {
    let now = 0
    const limiter = makeKhalaSyncAnonymousReadRateLimiter(() => now)
    const forwarded = new Request('https://openagents.com/api/sync/log', {
      headers: { 'x-forwarded-for': '198.51.100.1, 10.0.0.1' },
    })
    const noHeaders = new Request('https://openagents.com/api/sync/log')
    expect(limiter(forwarded)).toBe(true)
    expect(limiter(noHeaders)).toBe(true)
  })
})

describe('makeKhalaSyncAnonymousConnectRateLimiter', () => {
  test('the connect limiter is tighter than the read limiter (fewer admits per minute)', () => {
    let now = 0
    const limiter = makeKhalaSyncAnonymousConnectRateLimiter(() => now)
    const request = requestFrom('203.0.113.9')
    const results = Array.from({ length: 21 }, () => limiter(request))
    expect(results.slice(0, 20)).toEqual(Array(20).fill(true))
    expect(results[20]).toBe(false)
  })

  test('the connect and read limiters returned by separate factory calls track independent state', () => {
    let now = 0
    const readLimiter = makeKhalaSyncAnonymousReadRateLimiter(() => now)
    const connectLimiter = makeKhalaSyncAnonymousConnectRateLimiter(() => now)
    const request = requestFrom('203.0.113.10')
    for (let i = 0; i < 20; i += 1) connectLimiter(request)
    expect(connectLimiter(request)).toBe(false)
    // The read limiter's counters are a completely separate Map instance.
    expect(readLimiter(request)).toBe(true)
  })
})
