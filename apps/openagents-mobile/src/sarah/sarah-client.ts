import { fetch as expoFetch } from "expo/fetch"

import { drainSseBuffer, type SarahTurnResult } from "../screens/sarah-core"

/**
 * OpenAgents mobile GL-3 (#8649) — the EFFECTFUL half of the Sarah surface:
 * production `/sarah` API access, on-device session persistence, and the
 * bounded SSE stream with reconnect. This module owns every network/disk
 * side effect; the pure view program (`../screens/sarah-core.ts` +
 * `home-core.ts`) only ever sees typed intents/results.
 *
 * JS-only constraint (OTA rail): persistence uses `expo-file-system` because
 * its native module is ALREADY linked in the shipping runtime
 * (`ExpoFileSystem_privacy.bundle` in builds 111/112) — adding
 * AsyncStorage would change the native runtime and break over-the-air
 * delivery to the installed builds.
 */

export const SARAH_API_BASE = "https://openagents.com/sarah/api"

const SESSION_FILE = "sarah-session-v1.json"
const THREADS_FILE = "sarah-threads-v1.json"
export const MAX_PERSISTED_SARAH_THREADS = 5

export interface PersistedSarahSession {
  readonly prospectRef: string
  readonly threadId: string
  readonly entries: ReadonlyArray<{
    readonly key: string
    readonly role: "user" | "assistant"
    readonly text: string
  }>
}

/** A deliberately small, app-owned conversation index. Mobile never crawls
 * device storage: it remembers only conversations the user created here. */
export interface PersistedSarahThread extends PersistedSarahSession {
  readonly title: string
  readonly updatedAt: number
}

// expo-file-system modern API (SDK 57): File/Paths classes. Imported lazily
// inside functions so unit tests (bun, no native runtime) can import this
// module for its pure helpers without expo natives present.
const sessionFile = async () => {
  const { File, Paths } = await import("expo-file-system")
  return new File(Paths.document, SESSION_FILE)
}

const threadsFile = async () => {
  const { File, Paths } = await import("expo-file-system")
  return new File(Paths.document, THREADS_FILE)
}

const validPersistedEntries = (
  value: unknown,
): ReadonlyArray<PersistedSarahSession["entries"][number]> =>
  Array.isArray(value)
    ? value
        .filter(
          (entry): entry is PersistedSarahSession["entries"][number] =>
            typeof entry === "object" &&
            entry !== null &&
            typeof (entry as { key?: unknown }).key === "string" &&
            ((entry as { role?: unknown }).role === "user" ||
              (entry as { role?: unknown }).role === "assistant") &&
            typeof (entry as { text?: unknown }).text === "string",
        )
        .slice(-40)
    : []

export const loadPersistedSarahThreads = async (): Promise<ReadonlyArray<PersistedSarahThread>> => {
  try {
    const file = await threadsFile()
    if (!file.exists) return []
    const parsed = JSON.parse(await file.text()) as { threads?: unknown }
    if (!Array.isArray(parsed.threads)) return []
    return parsed.threads
      .filter(
        (thread): thread is PersistedSarahThread =>
          typeof thread === "object" &&
          thread !== null &&
          typeof (thread as { prospectRef?: unknown }).prospectRef === "string" &&
          typeof (thread as { threadId?: unknown }).threadId === "string" &&
          typeof (thread as { title?: unknown }).title === "string" &&
          typeof (thread as { updatedAt?: unknown }).updatedAt === "number",
      )
      .map((thread) => ({ ...thread, entries: validPersistedEntries(thread.entries) }))
      .sort((a, b) => b.updatedAt - a.updatedAt)
      .slice(0, MAX_PERSISTED_SARAH_THREADS)
  } catch {
    return []
  }
}

export const persistSarahThread = async (thread: PersistedSarahThread): Promise<ReadonlyArray<PersistedSarahThread>> => {
  const existing = await loadPersistedSarahThreads()
  const next = [
    { ...thread, title: thread.title.trim().slice(0, 96) || "New chat", entries: thread.entries.slice(-40) },
    ...existing.filter((item) => item.threadId !== thread.threadId),
  ]
    .sort((a, b) => b.updatedAt - a.updatedAt)
    .slice(0, MAX_PERSISTED_SARAH_THREADS)
  try {
    const file = await threadsFile()
    file.write(JSON.stringify({ threads: next }))
  } catch {
    // The current conversation remains usable when device persistence fails.
  }
  return next
}

export const loadPersistedSarahSession =
  async (): Promise<PersistedSarahSession | null> => {
    try {
      const file = await sessionFile()
      if (!file.exists) return null
      const raw = await file.text()
      const parsed = JSON.parse(raw) as Partial<PersistedSarahSession>
      if (
        typeof parsed.prospectRef !== "string" ||
        parsed.prospectRef.length === 0 ||
        typeof parsed.threadId !== "string" ||
        parsed.threadId.length === 0
      ) {
        return null
      }
      const entries = validPersistedEntries(parsed.entries)
      return { prospectRef: parsed.prospectRef, threadId: parsed.threadId, entries }
    } catch {
      // Unreadable persistence is treated as absent — never a crash.
      return null
    }
  }

export const persistSarahSession = async (
  session: PersistedSarahSession,
): Promise<void> => {
  try {
    const file = await sessionFile()
    file.write(JSON.stringify({ ...session, entries: session.entries.slice(-40) }))
  } catch {
    // Persistence is best-effort; the live session keeps working without it.
  }
}

const fetchWithTimeout = async (
  url: string,
  init: RequestInit,
  timeoutMs: number,
): Promise<Response> => {
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), timeoutMs)
  try {
    return await fetch(url, { ...init, signal: controller.signal })
  } finally {
    clearTimeout(timer)
  }
}

/** Mint (or server-side reuse) a prospect session. The ref is the durable
 * relationship identity; we persist it ourselves (no cookie jar dependency)
 * and send it explicitly on every turn. */
export const mintSarahProspectSession = async (): Promise<{
  readonly prospectRef: string
  readonly threadId: string
}> => {
  const response = await fetchWithTimeout(
    `${SARAH_API_BASE}/prospect/session`,
    { method: "POST" },
    10_000,
  )
  if (!response.ok) throw new Error(`prospect_session_http_${response.status}`)
  const data = (await response.json()) as {
    prospectRef?: string
    threadId?: string
  }
  if (typeof data.prospectRef !== "string" || data.prospectRef.length === 0) {
    throw new Error("prospect_session_malformed")
  }
  return {
    prospectRef: data.prospectRef,
    threadId:
      typeof data.threadId === "string" && data.threadId.length > 0
        ? data.threadId
        : `prospect:${data.prospectRef}`,
  }
}

/** One text turn against the SAME route the web composer uses
 * (`POST /sarah/api/eve/turn`); prospectRef rides in the body so the mobile
 * relationship never depends on cookies. */
export const sendSarahTurn = async (input: {
  readonly message: string
  readonly prospectRef: string | null
  readonly threadId: string | null
}): Promise<SarahTurnResult> => {
  const response = await fetchWithTimeout(
    `${SARAH_API_BASE}/eve/turn`,
    {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        message: input.message,
        ...(input.prospectRef === null ? {} : { prospectRef: input.prospectRef }),
        ...(input.threadId === null ? {} : { threadId: input.threadId }),
      }),
    },
    45_000,
  )
  if (!response.ok) throw new Error(`turn_http_${response.status}`)
  const data = (await response.json()) as {
    reply?: string
    modelPath?: string
    threadId?: string
  }
  return {
    ok: true,
    reply: typeof data.reply === "string" ? data.reply : "(no reply)",
    modelPath: typeof data.modelPath === "string" ? data.modelPath : null,
    threadId: typeof data.threadId === "string" ? data.threadId : null,
  }
}

// ---------------------------------------------------------------------------
// Bounded SSE stream with reconnect
// ---------------------------------------------------------------------------

export interface SarahStreamCallbacks {
  readonly onStatus: (
    phase: "connecting" | "live" | "reconnecting" | "unavailable",
  ) => void
  readonly onEvent: (event: {
    readonly type: string
    readonly role?: string
    readonly text?: string
    readonly title?: string
    readonly body?: string
  }) => void
}

const RECONNECT_BASE_MS = 1_000
const RECONNECT_MAX_MS = 15_000
/** The server writes `: hb` comments every 25s. A silent link longer than
 * this deadline means the connection is dead (an idle TCP socket can look
 * "connected" for minutes after the network drops) — treat it as lost and
 * reconnect. This keeps the SSE stream BOUNDED: liveness is proven by bytes,
 * never assumed. */
const READ_SILENCE_DEADLINE_MS = 40_000
/** After this many consecutive failed connects the typed phase degrades to
 * "unavailable" (the surface shows it honestly) — but the loop KEEPS trying;
 * recovery flips back to live without user action. */
const UNAVAILABLE_AFTER_FAILURES = 3

const sleep = (ms: number, signal: AbortSignal): Promise<void> =>
  new Promise((resolve) => {
    const timer = setTimeout(done, ms)
    function done() {
      signal.removeEventListener("abort", done)
      clearTimeout(timer)
      resolve()
    }
    signal.addEventListener("abort", done)
  })

/**
 * Long-lived SSE consumption of `/sarah/api/avatar/events?ref=<prospectRef>`
 * via expo/fetch response streaming (React Native's global fetch cannot
 * stream bodies; EventSource does not exist on RN). Runs until the signal
 * aborts. Typed transcript/card events are forwarded; connection lifecycle
 * is reported through onStatus with bounded exponential backoff reconnect.
 */
export const runSarahEventStream = async (input: {
  readonly prospectRef: string
  readonly signal: AbortSignal
  readonly callbacks: SarahStreamCallbacks
}): Promise<void> => {
  const { prospectRef, signal, callbacks } = input
  let failures = 0
  let everConnected = false
  while (!signal.aborted) {
    callbacks.onStatus(everConnected || failures > 0 ? "reconnecting" : "connecting")
    try {
      const response = await expoFetch(
        `${SARAH_API_BASE}/avatar/events?ref=${encodeURIComponent(prospectRef)}`,
        { headers: { accept: "text/event-stream" }, signal },
      )
      if (!response.ok || response.body === null) {
        throw new Error(`sse_http_${response.status}`)
      }
      // Bytes flowing (the server writes `: connected <ref>` immediately).
      failures = 0
      everConnected = true
      callbacks.onStatus("live")
      const reader = response.body.getReader()
      const decoder = new TextDecoder()
      let buffer = ""
      for (;;) {
        // Heartbeat watchdog: a read that stays silent past the deadline is
        // a dead link (missed heartbeat), not a healthy idle stream.
        const result = await Promise.race([
          reader.read(),
          new Promise<"silent">((resolve) => {
            setTimeout(() => resolve("silent"), READ_SILENCE_DEADLINE_MS)
          }),
        ])
        if (result === "silent") {
          try {
            await reader.cancel()
          } catch {
            // Cancellation of a dead reader is best-effort.
          }
          throw new Error("sse_heartbeat_lost")
        }
        const { done, value } = result
        if (done) break
        buffer += decoder.decode(value, { stream: true })
        buffer = drainSseBuffer(buffer, (data) => {
          try {
            const event = JSON.parse(data) as {
              type?: string
              role?: string
              text?: string
              title?: string
              body?: string
            }
            if (typeof event.type === "string") {
              callbacks.onEvent({
                type: event.type,
                ...(typeof event.role === "string" ? { role: event.role } : {}),
                ...(typeof event.text === "string" ? { text: event.text } : {}),
                ...(typeof event.title === "string" ? { title: event.title } : {}),
                ...(typeof event.body === "string" ? { body: event.body } : {}),
              })
            }
          } catch {
            // Malformed frames are dropped; the stream stays up.
          }
        })
        // Bounded buffer: a frame larger than 64 KiB is hostile/broken.
        if (buffer.length > 64 * 1024) buffer = ""
      }
      // Server closed (deploy/idle) — reconnect.
      throw new Error("sse_closed")
    } catch {
      if (signal.aborted) return
      failures += 1
      if (failures >= UNAVAILABLE_AFTER_FAILURES) {
        callbacks.onStatus("unavailable")
      }
      const backoff = Math.min(
        RECONNECT_MAX_MS,
        RECONNECT_BASE_MS * 2 ** Math.min(failures - 1, 4),
      )
      await sleep(backoff, signal)
    }
  }
}
