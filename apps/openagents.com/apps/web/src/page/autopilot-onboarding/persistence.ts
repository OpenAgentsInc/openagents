// Autopilot onboarding — browser-side session persistence + resume cursor
// (unauthenticated). An onboarding visitor has no account; the only identity is
// the client-minted `sessionId`. To survive a reload or a navigate-away, the
// conversation (transcript + accumulated Output Spec) and an in-flight stream
// cursor are persisted in `localStorage`, keyed by `sessionId`. On the next
// load the page rehydrates the transcript immediately, reconciles with the
// server (GET session), and — if a turn was mid-stream — resumes that turn from
// the durable log (the resume read).
//
// This module is SSR/headless-safe: every `localStorage` touch is guarded
// (`typeof window`/feature-detect) and wrapped so a private-mode/quota/corrupt
// failure degrades to "no stored session" rather than throwing. The stored
// record shape is an Effect Schema, decoded defensively on read — a malformed
// or schema-drifted blob is treated as absent (clean fresh start), never a
// crash.
//
// Privacy: we persist ONLY the conversation/spec the visitor themselves
// entered (transcript text + the derived Output Spec) plus the minimal cursor
// needed to resume a stream. No secrets, tokens, or payment material. The
// transcript is capped so a long conversation cannot grow the blob unbounded.

import { Option, Schema as S } from 'effect'

import {
  FlowOutputSpec,
  FlowTurn,
  type FlowOutputSpec as FlowOutputSpecType,
  type FlowTurn as FlowTurnType,
} from './flow'

// STORAGE KEY -------------------------------------------------------------

// One record per browser, holding the single most-recent onboarding session.
// The unauthenticated flow is single-session by design (one `/autopilot`
// conversation at a time); a "start over" affordance clears it.
export const ONBOARDING_STORAGE_KEY = 'oa.autopilot.onboarding.v1'

// Cap the persisted transcript so the blob stays bounded even for a very long
// conversation. We keep the most recent turns (the tail), which is what a
// resuming reader needs; the server's GET session remains the authority for the
// full transcript and reconciles on mount.
export const MAX_STORED_TRANSCRIPT_TURNS = 80

// STORED RECORD SCHEMA ----------------------------------------------------

// The in-flight cursor for a turn that was mid-stream when the tab went away.
// `streamId` / `turnIndex` come from the server `event: stream` handshake frame;
// `replySoFar` is the partial assistant text already rendered; `lastOffset` is
// the durable-log byte offset to resume from when one was observed (the LIVE
// pass-through may not expose per-delta offsets — then it stays absent and the
// resume read starts from offset 0, which replays the whole in-flight turn).
export const StoredInFlight = S.Struct({
  streamId: S.String,
  turnIndex: S.Int,
  replySoFar: S.String,
  lastOffset: S.optionalKey(S.NullOr(S.String)),
})
export type StoredInFlight = typeof StoredInFlight.Type

// The session status mirrored from the server (`interviewing | complete`).
// Persisted so a returning tab can render the completed state without waiting
// for the reconcile fetch.
export const StoredOnboardingStatus = S.Literals(['interviewing', 'complete'])
export type StoredOnboardingStatus = typeof StoredOnboardingStatus.Type

export const StoredOnboardingSession = S.Struct({
  sessionId: S.String,
  vertical: S.optionalKey(S.NullOr(S.String)),
  status: S.optionalKey(S.NullOr(StoredOnboardingStatus)),
  transcript: S.Array(FlowTurn),
  outputSpec: S.optionalKey(FlowOutputSpec),
  inFlight: S.optionalKey(S.NullOr(StoredInFlight)),
  updatedAt: S.Int,
})
export type StoredOnboardingSession = typeof StoredOnboardingSession.Type

const StoredOnboardingSessionFromJson = S.fromJsonString(StoredOnboardingSession)

// PURE TRANSFORMS ---------------------------------------------------------

// Keep only the most recent `MAX_STORED_TRANSCRIPT_TURNS` turns. Pure; the
// caller decides when to apply it (every persist).
export const capTranscript = (
  transcript: ReadonlyArray<FlowTurnType>,
): ReadonlyArray<FlowTurnType> =>
  transcript.length <= MAX_STORED_TRANSCRIPT_TURNS
    ? transcript
    : transcript.slice(transcript.length - MAX_STORED_TRANSCRIPT_TURNS)

// Encode a stored record to its JSON string. Pure + total (the schema encode
// never fails for in-domain data). Caps the transcript defensively so a caller
// that forgot cannot persist an unbounded blob.
export const encodeStoredSession = (session: StoredOnboardingSession): string =>
  S.encodeSync(StoredOnboardingSessionFromJson)({
    ...session,
    transcript: capTranscript(session.transcript),
  })

// Decode a stored JSON string into a record, defensively. A malformed or
// schema-drifted blob yields `none` (treated as absent — a clean fresh start).
export const decodeStoredSession = (
  raw: string,
): Option.Option<StoredOnboardingSession> =>
  S.decodeUnknownOption(StoredOnboardingSessionFromJson)(raw)

// STORAGE PORT ------------------------------------------------------------

// A narrow key/value port over the durable browser store. Injectable so the
// full client↔server resume handshake is exercisable headlessly (an in-memory
// port stands in for `window.localStorage` in tests), and so the production
// store can be swapped without rewriting the read/write/clear logic. Every
// method is total: an implementation MUST swallow its own quota/private-mode/
// SSR failures and degrade to "absent" rather than throw, so the callers below
// stay branch-free and a storage fault can never crash the page.
export type OnboardingStoragePort = Readonly<{
  // The raw stored string for the key, or `null` when absent/unavailable.
  get: (key: string) => string | null
  // Persist the value (best effort).
  set: (key: string, value: string) => void
  // Remove the value (best effort).
  remove: (key: string) => void
}>

// Feature-detect a usable `localStorage`. Returns the store or `undefined` in
// SSR/headless/private-mode contexts where access throws or is absent. Probing
// access (not just `typeof`) catches Safari private mode, where the API exists
// but every call throws.
const maybeLocalStorage = (): Storage | undefined => {
  try {
    if (typeof globalThis === 'undefined') {
      return undefined
    }
    const store = (globalThis as { localStorage?: Storage }).localStorage
    if (store === undefined || store === null) {
      return undefined
    }
    // Touch the API so private-mode/disabled storage surfaces here, not later.
    const probe = '__oa_probe__'
    store.setItem(probe, probe)
    store.removeItem(probe)
    return store
  } catch {
    return undefined
  }
}

// The production port: `window.localStorage`, with every touch guarded so a
// quota/disabled/SSR failure degrades to "absent" instead of throwing.
export const localStorageOnboardingStoragePort: OnboardingStoragePort = {
  get: key => {
    const store = maybeLocalStorage()
    if (store === undefined) {
      return null
    }
    try {
      return store.getItem(key)
    } catch {
      return null
    }
  },
  set: (key, value) => {
    const store = maybeLocalStorage()
    if (store === undefined) {
      return
    }
    try {
      store.setItem(key, value)
    } catch {
      // ignore — best effort
    }
  },
  remove: key => {
    const store = maybeLocalStorage()
    if (store === undefined) {
      return
    }
    try {
      store.removeItem(key)
    } catch {
      // ignore — best effort
    }
  },
}

// A simple in-memory port over a `Map`, for headless tests (and any non-browser
// caller). Lets the full resume handshake — persist on stream, reload, restore,
// reconcile, resume — run with no DOM. Construct one per simulated browser.
export const makeMemoryOnboardingStoragePort = (
  backing: Map<string, string> = new Map(),
): OnboardingStoragePort => ({
  get: key => backing.get(key) ?? null,
  set: (key, value) => {
    backing.set(key, value)
  },
  remove: key => {
    backing.delete(key)
  },
})

// The active port. Defaults to `window.localStorage`; a test (or a future
// non-browser host) swaps it via `setOnboardingStoragePort`. The read/write/
// clear helpers below resolve the port lazily on each call, so a swap installed
// before the Foldkit command runs takes effect (the commands are plain
// `Effect.sync` over these helpers — they do not capture the port at define
// time).
let activeOnboardingStoragePort: OnboardingStoragePort =
  localStorageOnboardingStoragePort

// Install the active storage port. Returns a restore function that reinstates
// the previous port, so a test can scope the swap and leave the global clean.
export const setOnboardingStoragePort = (
  port: OnboardingStoragePort,
): (() => void) => {
  const previous = activeOnboardingStoragePort
  activeOnboardingStoragePort = port
  return () => {
    activeOnboardingStoragePort = previous
  }
}

// READ/WRITE/CLEAR --------------------------------------------------------

// Read + decode the stored session, defensively. Absent storage, a missing
// record, or a corrupt/drifted blob all yield `none` (clean fresh start). On a
// corrupt blob the bad value is cleared so it cannot wedge future loads. Reads
// through the active port (overridable for headless tests).
export const readStoredSession = (
  port: OnboardingStoragePort = activeOnboardingStoragePort,
): Option.Option<StoredOnboardingSession> => {
  const raw = port.get(ONBOARDING_STORAGE_KEY)
  if (raw === null) {
    return Option.none()
  }
  return decodeStoredSession(raw).pipe(
    Option.match({
      onNone: () => {
        // Corrupt/drifted: clear it so it does not wedge the next load.
        port.remove(ONBOARDING_STORAGE_KEY)
        return Option.none<StoredOnboardingSession>()
      },
      onSome: session => Option.some(session),
    }),
  )
}

// Persist a stored session, defensively. A quota/disabled-storage failure is
// swallowed by the port (persistence is best-effort; the server remains the
// authority). Writes through the active port (overridable for headless tests).
export const writeStoredSession = (
  session: StoredOnboardingSession,
  port: OnboardingStoragePort = activeOnboardingStoragePort,
): void => {
  port.set(ONBOARDING_STORAGE_KEY, encodeStoredSession(session))
}

// Clear the stored session (start over / expired / unknown). Defensive. Clears
// through the active port (overridable for headless tests).
export const clearStoredSession = (
  port: OnboardingStoragePort = activeOnboardingStoragePort,
): void => {
  port.remove(ONBOARDING_STORAGE_KEY)
}

// GET-SESSION DECODE ------------------------------------------------------

// The server's GET /api/autopilot/onboarding/{sessionId} response: the
// authoritative transcript + status + outputSpec + turnCount. Decoded at the
// client boundary; a drift fails loudly (caught by the caller's catch).
export const OnboardingSessionResponse = S.Struct({
  sessionId: S.String,
  status: StoredOnboardingStatus,
  turnCount: S.Int,
  transcript: S.Array(FlowTurn),
  outputSpec: FlowOutputSpec,
})
export type OnboardingSessionResponse = typeof OnboardingSessionResponse.Type

// Build a stored record from the live flow model fields. Centralizes the shape
// so the update sites stay declarative.
export const storedSessionFromParts = (parts: {
  sessionId: string
  vertical: string | null
  status: StoredOnboardingStatus | null
  transcript: ReadonlyArray<FlowTurnType>
  outputSpec: FlowOutputSpecType
  inFlight: StoredInFlight | null
  updatedAt: number
}): StoredOnboardingSession => ({
  sessionId: parts.sessionId,
  vertical: parts.vertical,
  status: parts.status,
  transcript: capTranscript(parts.transcript),
  outputSpec: parts.outputSpec,
  inFlight: parts.inFlight,
  updatedAt: parts.updatedAt,
})
