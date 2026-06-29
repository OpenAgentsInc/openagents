import type { NodeStateMessage, SessionEventRow } from "../shared/rpc.js"
import { externalSessionRefsFromEventRows } from "./pylon-control.js"

export function mergeSessionEventRows(
  current: readonly SessionEventRow[],
  incoming: SessionEventRow,
): SessionEventRow[] {
  const byIndex = new Map<number, SessionEventRow>()
  for (const event of current) byIndex.set(event.eventIndex, event)
  byIndex.set(incoming.eventIndex, incoming)
  return [...byIndex.values()]
    .sort((left, right) => left.eventIndex - right.eventIndex)
    .slice(-100)
}

export function sessionRefsToStream(message: NodeStateMessage): string[] {
  const refs = new Set<string>()
  for (const session of message.sessions) {
    if (session.state === "running") refs.add(session.sessionRef)
  }
  for (const events of Object.values(message.events ?? {})) {
    for (const ref of externalSessionRefsFromEventRows(events)) refs.add(ref)
  }
  for (const artifact of Object.values(message.artifacts ?? {})) {
    const ref = artifact.detail?.externalSessionRef
    if (typeof ref === "string" && ref.trim() !== "") refs.add(ref)
  }
  return [...refs]
}

function consumeSseBuffer(
  buffer: string,
  emit: (payload: string) => void,
): string {
  const frames = buffer.split("\n\n")
  const rest = frames.pop() ?? ""
  for (const frame of frames) {
    for (const line of frame.split("\n")) {
      if (line.startsWith("data: ")) emit(line.slice(6))
    }
  }
  return rest
}

function eventRowFromSsePayload(payload: string): SessionEventRow | null {
  let parsed: any
  try {
    parsed = JSON.parse(payload)
  } catch {
    return null
  }
  if (parsed === null || typeof parsed !== "object") return null
  return {
    eventIndex: Number(parsed.eventIndex ?? 0),
    phase: String(parsed.phase ?? "?"),
    state: String(parsed.state ?? "?"),
    observedAt: String(parsed.observedAt ?? ""),
    detail:
      typeof parsed.messageText === "string"
        ? parsed.messageText
        : typeof parsed.detail === "string"
          ? parsed.detail
          : "",
    ...(typeof parsed.messageFull === "string"
      ? { full: parsed.messageFull }
      : typeof parsed.full === "string"
        ? { full: parsed.full }
        : {}),
  }
}

export type SessionEventStreamer = {
  reconcile(message: NodeStateMessage): void
  watch(sessionRef: string): void
  stop(): void
}

export function createSessionEventStreamer(input: {
  baseUrl: string
  tokenProvider: () => Promise<string | null>
  fetchFn?: typeof fetch
  onEvent: (sessionRef: string, event: SessionEventRow) => void
}): SessionEventStreamer {
  const fetchFn = input.fetchFn ?? fetch
  const baseUrl = input.baseUrl.replace(/\/+$/, "")
  const active = new Map<string, AbortController>()
  const watched = new Set<string>()
  let desired = new Set<string>()

  const start = (sessionRef: string): void => {
    if (active.has(sessionRef)) return
    const abort = new AbortController()
    active.set(sessionRef, abort)
    void (async () => {
      try {
        const token = await input.tokenProvider()
        if (token === null || abort.signal.aborted) return
        const response = await fetchFn(`${baseUrl}/sessions/${encodeURIComponent(sessionRef)}/events`, {
          headers: { Authorization: `Bearer ${token}` },
          signal: abort.signal,
        })
        if (!response.ok || !response.body) return
        const reader = response.body.getReader()
        const decoder = new TextDecoder()
        let buffer = ""
        while (!abort.signal.aborted) {
          const { done, value } = await reader.read()
          if (done) break
          buffer += decoder.decode(value, { stream: true })
          buffer = consumeSseBuffer(buffer, (payload) => {
            const row = eventRowFromSsePayload(payload)
            if (row !== null) input.onEvent(sessionRef, row)
          })
        }
      } catch {
        // Polling stays the fallback; streaming must never break the shell.
      } finally {
        active.delete(sessionRef)
        if (desired.has(sessionRef)) {
          const retry = setTimeout(() => {
            if (desired.has(sessionRef)) start(sessionRef)
          }, 1000)
          ;(retry as { unref?: () => void }).unref?.()
        }
      }
    })()
  }

  const stopRef = (sessionRef: string): void => {
    const abort = active.get(sessionRef)
    if (abort === undefined) return
    abort.abort()
    active.delete(sessionRef)
  }

  return {
    reconcile(message) {
      for (const session of message.sessions) {
        if (session.state !== "running") watched.delete(session.sessionRef)
      }
      desired = new Set([...sessionRefsToStream(message), ...watched])
      for (const ref of desired) start(ref)
      for (const ref of [...active.keys()]) {
        if (!desired.has(ref)) stopRef(ref)
      }
    },
    watch(sessionRef) {
      if (sessionRef.trim() === "") return
      watched.add(sessionRef)
      desired.add(sessionRef)
      start(sessionRef)
    },
    stop() {
      watched.clear()
      desired = new Set()
      for (const ref of [...active.keys()]) stopRef(ref)
    },
  }
}
