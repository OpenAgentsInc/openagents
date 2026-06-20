// Chat-World live subscriptions (P1 #5736 + P2 #5737).
//
// Browser-side feeds for the "agent MMORPG" scene behind chat
// (docs/launch/2026-06-20-agent-mmorpg-hud-autopilot-audit-and-plan.md §3.3):
//   - subscribePylonScene(dispatch)      P1: poll /api/public/pylon-stats →
//                                        ChatWorldPylonScene (live nodes + growth)
//   - subscribePaymentParticles(dispatch) P2: SSE /api/public/activity-timeline/
//                                        stream (poll fallback) → PaymentParticle
//
// Both are FLAG-GATED (default OFF) and return an unsubscribe() the scene calls
// on teardown. They are intentionally NOT registered in the static Foldkit
// `subscriptions` (subscriptions.ts) — the P0 scene owns its own lifecycle and
// will call these when the scene mounts (see the TODO in subscriptions.ts).
//
// "browser UA": these run inside the Electrobun webview and fetch the public
// openagents.com endpoints directly, so requests carry the webview's browser
// User-Agent (no node/bun UA), matching how the public site is served.
//
// Evidence-bound (§5): subscribePaymentParticles only ever dispatches particles
// that carry a real sourceRef (activityEventToParticle drops the rest).

import {
  activityEventToParticle,
  chatWorldFlags,
  projectChatWorldPylonScene,
  type ActivityEvent,
  type ChatWorldPylonScene,
  type PaymentParticle,
} from "../shared/chat-world-scene"
import type { PylonStatsSnapshot } from "../shared/pylon-network-scene"

const PUBLIC_BASE_URL = "https://openagents.com"
const PYLON_STATS_PATH = "/api/public/pylon-stats"
const ACTIVITY_POLL_PATH = "/api/public/activity-timeline"
const ACTIVITY_STREAM_PATH = "/api/public/activity-timeline/stream"

const PYLON_POLL_INTERVAL_MS = 4_000
const ACTIVITY_POLL_INTERVAL_MS = 5_000

export type Unsubscribe = () => void

const noop: Unsubscribe = () => {}

// Injectable seams so the subscriptions are testable headlessly.
export type ChatWorldSubscriptionDeps = {
  readonly baseUrl?: string
  readonly fetchFn?: typeof fetch
  /** EventSource ctor (browser global); omit to use the platform one. */
  readonly eventSourceCtor?: typeof EventSource
  readonly setInterval?: (handler: () => void, ms: number) => unknown
  readonly clearInterval?: (handle: unknown) => void
  /** override flags (default: chatWorldFlags() from globalThis.__OA_FLAGS). */
  readonly flags?: { readonly CHAT_WORLD_SCENE?: boolean; readonly CHAT_WORLD_PAYMENTS?: boolean }
}

const resolveBaseUrl = (deps?: ChatWorldSubscriptionDeps): string =>
  (deps?.baseUrl ?? PUBLIC_BASE_URL).replace(/\/+$/, "")

const resolveSetInterval = (
  deps?: ChatWorldSubscriptionDeps,
): ((handler: () => void, ms: number) => unknown) =>
  deps?.setInterval ??
  ((handler, ms) => (globalThis as unknown as { setInterval: (h: () => void, m: number) => unknown }).setInterval(handler, ms))

const resolveClearInterval = (
  deps?: ChatWorldSubscriptionDeps,
): ((handle: unknown) => void) =>
  deps?.clearInterval ??
  ((handle) => (globalThis as unknown as { clearInterval: (h: unknown) => void }).clearInterval(handle))

// ─────────────────────────────────────────────────────────────────────────────
// P1 — live pylons
// ─────────────────────────────────────────────────────────────────────────────

export type PylonSceneDispatch = (scene: ChatWorldPylonScene) => void

// Poll pylon-stats and push a projected scene to `dispatch`. Fail-soft: a fetch
// error pushes the honest zero-state (projectChatWorldPylonScene(null)) rather
// than throwing or freezing the last snapshot. Returns unsubscribe().
export const subscribePylonScene = (
  dispatch: PylonSceneDispatch,
  deps?: ChatWorldSubscriptionDeps,
): Unsubscribe => {
  const flags = deps?.flags ?? chatWorldFlags()
  if (flags.CHAT_WORLD_SCENE !== true) return noop

  const baseUrl = resolveBaseUrl(deps)
  const fetchFn = deps?.fetchFn ?? fetch
  const url = `${baseUrl}${PYLON_STATS_PATH}`
  let stopped = false

  const poll = async (): Promise<void> => {
    try {
      const response = await fetchFn(url, { headers: { accept: "application/json" } })
      if (stopped) return
      if (!response.ok) {
        dispatch(projectChatWorldPylonScene(null))
        return
      }
      const snapshot = (await response.json()) as PylonStatsSnapshot
      if (stopped) return
      dispatch(projectChatWorldPylonScene(snapshot))
    } catch {
      if (!stopped) dispatch(projectChatWorldPylonScene(null))
    }
  }

  void poll()
  const handle = resolveSetInterval(deps)(() => void poll(), PYLON_POLL_INTERVAL_MS)

  return () => {
    stopped = true
    resolveClearInterval(deps)(handle)
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// P2 — payment particles
// ─────────────────────────────────────────────────────────────────────────────

export type PaymentParticleDispatch = (particle: PaymentParticle) => void

// Parse one SSE `data:` payload ({ event: <ActivityEvent> }) into a particle.
// Tolerant of either { event: {...} } (the worker's frame shape) or a bare event
// object. Returns null when it is not an honestly-renderable payment.
export const parseActivityStreamData = (raw: string): PaymentParticle | null => {
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return null
  }
  const candidate =
    parsed && typeof parsed === "object" && "event" in (parsed as Record<string, unknown>)
      ? (parsed as { event: unknown }).event
      : parsed
  if (!candidate || typeof candidate !== "object") return null
  const event = candidate as ActivityEvent
  if (typeof event.eventRef !== "string" || typeof event.kind !== "string") return null
  return activityEventToParticle(event)
}

// Backfill / poll: pull the activity-timeline envelope once and push particles
// for every payment event it carries (used for "backfill last N on connect" and
// as the no-EventSource fallback).
const pollActivityOnce = async (
  baseUrl: string,
  fetchFn: typeof fetch,
  dispatch: PaymentParticleDispatch,
  isStopped: () => boolean,
): Promise<void> => {
  try {
    const response = await fetchFn(`${baseUrl}${ACTIVITY_POLL_PATH}`, {
      headers: { accept: "application/json" },
    })
    if (isStopped() || !response.ok) return
    const envelope = (await response.json()) as { events?: ReadonlyArray<ActivityEvent> }
    if (isStopped()) return
    for (const event of envelope.events ?? []) {
      const particle = activityEventToParticle(event)
      if (particle) dispatch(particle)
    }
  } catch {
    // fail-soft: no particles rather than throwing
  }
}

// Subscribe to the activity SSE stream and push a PaymentParticle per real
// money event. Prefers EventSource (live SSE); if EventSource is unavailable,
// falls back to polling the envelope. Always does one backfill poll on connect
// so the scene starts populated. Returns unsubscribe().
export const subscribePaymentParticles = (
  dispatch: PaymentParticleDispatch,
  deps?: ChatWorldSubscriptionDeps,
): Unsubscribe => {
  const flags = deps?.flags ?? chatWorldFlags()
  if (flags.CHAT_WORLD_PAYMENTS !== true) return noop

  const baseUrl = resolveBaseUrl(deps)
  const fetchFn = deps?.fetchFn ?? fetch
  let stopped = false
  const isStopped = (): boolean => stopped

  // Backfill last N events on connect (evidence-bound; non-payments dropped).
  void pollActivityOnce(baseUrl, fetchFn, dispatch, isStopped)

  const EventSourceCtor =
    deps?.eventSourceCtor ??
    (globalThis as unknown as { EventSource?: typeof EventSource }).EventSource

  // No EventSource (headless / older webview): poll the envelope on an interval.
  if (typeof EventSourceCtor !== "function") {
    const handle = resolveSetInterval(deps)(
      () => void pollActivityOnce(baseUrl, fetchFn, dispatch, isStopped),
      ACTIVITY_POLL_INTERVAL_MS,
    )
    return () => {
      stopped = true
      resolveClearInterval(deps)(handle)
    }
  }

  const source = new EventSourceCtor(`${baseUrl}${ACTIVITY_STREAM_PATH}`)
  // The worker frames each event with `event: <kind>` and data { event }. We
  // listen to the two payment kinds by name, plus the default `message` handler
  // as a safety net for servers that do not set the SSE event field.
  const onMessage = (raw: unknown): void => {
    if (stopped) return
    const data = (raw as { data?: string }).data
    if (typeof data !== "string") return
    const particle = parseActivityStreamData(data)
    if (particle) dispatch(particle)
  }
  source.addEventListener("real_bitcoin_moved", onMessage as EventListener)
  source.addEventListener("settlement_recorded", onMessage as EventListener)
  source.addEventListener("message", onMessage as EventListener)

  return () => {
    stopped = true
    source.removeEventListener("real_bitcoin_moved", onMessage as EventListener)
    source.removeEventListener("settlement_recorded", onMessage as EventListener)
    source.removeEventListener("message", onMessage as EventListener)
    source.close()
  }
}
