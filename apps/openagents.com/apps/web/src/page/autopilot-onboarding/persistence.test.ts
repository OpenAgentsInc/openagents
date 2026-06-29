import { Option } from 'effect'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

import {
  MAX_STORED_TRANSCRIPT_TURNS,
  ONBOARDING_STORAGE_KEY,
  type StoredOnboardingSession,
  capTranscript,
  clearStoredSession,
  decodeStoredSession,
  encodeStoredSession,
  makeMemoryOnboardingStoragePort,
  readStoredSession,
  setOnboardingStoragePort,
  storedSessionFromParts,
  writeStoredSession,
} from './persistence'

const sampleSession = (
  overrides: Partial<StoredOnboardingSession> = {},
): StoredOnboardingSession =>
  storedSessionFromParts({
    sessionId: 'ob_abc123',
    vertical: null,
    status: 'interviewing',
    transcript: [
      { role: 'user', content: 'I run a bakery' },
      { role: 'assistant', content: 'Great — what do you want done?' },
    ],
    outputSpec: { business: 'Acme Bakery' },
    inFlight: null,
    updatedAt: 1_700_000_000_000,
    ...overrides,
  })

describe('autopilot onboarding persistence — pure transforms', () => {
  test('encode then decode round-trips a stored session', () => {
    const session = sampleSession()
    const encoded = encodeStoredSession(session)
    const decoded = decodeStoredSession(encoded)

    expect(Option.isSome(decoded)).toBe(true)
    expect(Option.getOrThrow(decoded)).toEqual(session)
  })

  test('decode of a corrupt or drifted blob yields none (never throws)', () => {
    expect(Option.isNone(decodeStoredSession('not json at all'))).toBe(true)
    expect(Option.isNone(decodeStoredSession('{"sessionId": 42}'))).toBe(true)
    expect(Option.isNone(decodeStoredSession('{}'))).toBe(true)
  })

  test('round-trips an in-flight cursor for stream resume', () => {
    const session = sampleSession({
      inFlight: {
        streamId: 'onboarding:ob_abc123:1',
        turnIndex: 1,
        replySoFar: 'Sure, here is the plan so',
        lastOffset: '128',
      },
    })
    const decoded = decodeStoredSession(encodeStoredSession(session))
    expect(Option.getOrThrow(decoded).inFlight).toEqual(session.inFlight)
  })

  test('caps the transcript to the most recent turns', () => {
    const longTranscript = Array.from(
      { length: MAX_STORED_TRANSCRIPT_TURNS + 10 },
      (_, index) => ({ role: 'user' as const, content: `turn ${index}` }),
    )
    const capped = capTranscript(longTranscript)
    expect(capped.length).toBe(MAX_STORED_TRANSCRIPT_TURNS)
    // Keeps the TAIL (most recent), not the head.
    expect(capped[capped.length - 1]?.content).toBe(
      `turn ${MAX_STORED_TRANSCRIPT_TURNS + 9}`,
    )
    expect(capped[0]?.content).toBe('turn 10')
  })

  test('encode applies the transcript cap defensively', () => {
    const session = sampleSession({
      transcript: Array.from(
        { length: MAX_STORED_TRANSCRIPT_TURNS + 5 },
        (_, index) => ({ role: 'assistant' as const, content: `r${index}` }),
      ),
    })
    const decoded = Option.getOrThrow(
      decodeStoredSession(encodeStoredSession(session)),
    )
    expect(decoded.transcript.length).toBe(MAX_STORED_TRANSCRIPT_TURNS)
  })
})

// A minimal in-memory localStorage shim. The headless test environment's
// `localStorage` is not a fully functional Storage, so the wrapper tests install
// their own; the production code reaches `globalThis.localStorage` exactly as it
// would in a browser.
class MemoryStorage {
  private store = new Map<string, string>()
  getItem(key: string): string | null {
    return this.store.has(key) ? (this.store.get(key) as string) : null
  }
  setItem(key: string, value: string): void {
    this.store.set(key, value)
  }
  removeItem(key: string): void {
    this.store.delete(key)
  }
  clear(): void {
    this.store.clear()
  }
}

let originalLocalStorage: PropertyDescriptor | undefined

const installMemoryStorage = (storage: unknown): void => {
  Object.defineProperty(globalThis, 'localStorage', {
    configurable: true,
    value: storage,
  })
}

describe('autopilot onboarding persistence — localStorage wrapper', () => {
  beforeEach(() => {
    originalLocalStorage = Object.getOwnPropertyDescriptor(
      globalThis,
      'localStorage',
    )
    installMemoryStorage(new MemoryStorage())
  })

  afterEach(() => {
    if (originalLocalStorage !== undefined) {
      Object.defineProperty(globalThis, 'localStorage', originalLocalStorage)
    }
    vi.restoreAllMocks()
  })

  test('write then read returns the same session', () => {
    const session = sampleSession()
    writeStoredSession(session)

    const read = readStoredSession()
    expect(Option.isSome(read)).toBe(true)
    expect(Option.getOrThrow(read)).toEqual(session)
  })

  test('read of an absent record returns none', () => {
    expect(Option.isNone(readStoredSession())).toBe(true)
  })

  test('clear removes the stored record', () => {
    writeStoredSession(sampleSession())
    expect(Option.isSome(readStoredSession())).toBe(true)

    clearStoredSession()
    expect(Option.isNone(readStoredSession())).toBe(true)
  })

  test('a corrupt stored blob reads as none AND is cleared', () => {
    localStorage.setItem(ONBOARDING_STORAGE_KEY, '{"broken": true')
    expect(Option.isNone(readStoredSession())).toBe(true)
    // The corrupt value is purged so it cannot wedge the next load.
    expect(localStorage.getItem(ONBOARDING_STORAGE_KEY)).toBeNull()
  })

  test('a write failure (quota) is swallowed, not thrown', () => {
    const throwingStorage = new MemoryStorage()
    throwingStorage.setItem = () => {
      throw new Error('QuotaExceededError')
    }
    installMemoryStorage(throwingStorage)
    // The first setItem call is the probe in maybeLocalStorage; it throws, so
    // the wrapper degrades to a no-op store. No exception escapes.
    expect(() => writeStoredSession(sampleSession())).not.toThrow()
  })
})

describe('autopilot onboarding persistence — injectable storage port', () => {
  let restore: (() => void) | undefined
  afterEach(() => {
    restore?.()
    restore = undefined
  })

  test('an installed in-memory port backs read/write/clear with no DOM', () => {
    const backing = new Map<string, string>()
    restore = setOnboardingStoragePort(makeMemoryOnboardingStoragePort(backing))

    // Nothing stored yet.
    expect(Option.isNone(readStoredSession())).toBe(true)

    // Write goes to the in-memory port (no window.localStorage touched).
    writeStoredSession(sampleSession())
    expect(backing.get(ONBOARDING_STORAGE_KEY)).toBeTruthy()
    const read = readStoredSession()
    expect(Option.isSome(read)).toBe(true)
    expect(Option.getOrThrow(read).sessionId).toBe('ob_abc123')

    // Clear removes it from the same port.
    clearStoredSession()
    expect(backing.has(ONBOARDING_STORAGE_KEY)).toBe(false)
    expect(Option.isNone(readStoredSession())).toBe(true)
  })

  test('the explicit port argument overrides the active port (no global swap needed)', () => {
    const portA = makeMemoryOnboardingStoragePort()
    writeStoredSession(sampleSession(), portA)
    expect(Option.isSome(readStoredSession(portA))).toBe(true)
    // A fresh, unrelated port sees nothing.
    expect(Option.isNone(readStoredSession(makeMemoryOnboardingStoragePort()))).toBe(
      true,
    )
  })

  test('setOnboardingStoragePort returns a restore that reinstates the previous port', () => {
    const first = makeMemoryOnboardingStoragePort()
    const restoreFirst = setOnboardingStoragePort(first)
    writeStoredSession(sampleSession())
    const second = makeMemoryOnboardingStoragePort()
    const restoreSecond = setOnboardingStoragePort(second)
    // The active port is now `second`, which is empty.
    expect(Option.isNone(readStoredSession())).toBe(true)
    restoreSecond()
    // Back to `first`, which still holds the record.
    expect(Option.isSome(readStoredSession())).toBe(true)
    restoreFirst()
  })

  test('a corrupt blob in the port reads as none AND is purged from the port', () => {
    const backing = new Map<string, string>([
      [ONBOARDING_STORAGE_KEY, '{"broken": true'],
    ])
    restore = setOnboardingStoragePort(makeMemoryOnboardingStoragePort(backing))
    expect(Option.isNone(readStoredSession())).toBe(true)
    expect(backing.has(ONBOARDING_STORAGE_KEY)).toBe(false)
  })
})
