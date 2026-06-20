// Control client for `pylon attach` (issue #4740): consumes a node's
// snapshot + live PylonEvent SSE stream with exponential-backoff reconnect
// (1s -> 30s, the opencode context/sdk.tsx pattern), and sends typed
// commands over the HTTP API.

import { Effect, type Scope } from "effect"
import type { PylonEvent } from "./state.js"
import type { ControlCommand, PylonSnapshot } from "./control-server.js"

export interface ControlClientHandlers {
  onSnapshot: (snapshot: PylonSnapshot) => void
  onEvent: (event: PylonEvent) => void
  onStatus: (status: "connected" | "reconnecting", detail?: string) => void
}

export function nextBackoffMs(previous: number): number {
  return Math.min(previous <= 0 ? 1000 : previous * 2, 30_000)
}

// Parses complete SSE frames out of a growing buffer; returns the remaining
// partial buffer. Comment frames (": ping") are ignored.
export function consumeSseBuffer(
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

async function streamOnce(
  baseUrl: string,
  token: string,
  handlers: ControlClientHandlers,
  signal: AbortSignal,
): Promise<void> {
  const response = await fetch(`${baseUrl}/events`, {
    headers: { authorization: `Bearer ${token}` },
    signal,
  })
  if (!response.ok || !response.body) {
    throw new Error(`events stream rejected: HTTP ${response.status}`)
  }
  handlers.onStatus("connected")
  const reader = response.body.getReader()
  const decoder = new TextDecoder()
  let buffer = ""
  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    buffer += decoder.decode(value, { stream: true })
    buffer = consumeSseBuffer(buffer, (payload) => {
      try {
        const parsed = JSON.parse(payload) as PylonSnapshot | PylonEvent
        if (parsed.type === "snapshot") handlers.onSnapshot(parsed as PylonSnapshot)
        else handlers.onEvent(parsed as PylonEvent)
      } catch {
        // Malformed frame: skip rather than kill the stream.
      }
    })
  }
  throw new Error("events stream ended")
}

// Runs the attach stream as a scoped fiber: reconnects with backoff forever
// until the Scope closes.
export const runControlClient = (
  baseUrl: string,
  token: string,
  handlers: ControlClientHandlers,
): Effect.Effect<void, never, Scope.Scope> =>
  Effect.gen(function* () {
    const abort = new AbortController()
    yield* Effect.addFinalizer(() => Effect.sync(() => abort.abort()))
    yield* Effect.forkScoped(
      Effect.gen(function* () {
        let backoff = 0
        while (!abort.signal.aborted) {
          const failure = yield* Effect.promise(async () => {
            try {
              await streamOnce(baseUrl, token, handlers, abort.signal)
              return null
            } catch (error) {
              return error instanceof Error ? error.message : String(error)
            }
          })
          if (abort.signal.aborted) return
          backoff = nextBackoffMs(backoff)
          handlers.onStatus("reconnecting", `${failure ?? "stream closed"}; retrying in ${backoff / 1000}s`)
          yield* Effect.sleep(`${backoff} millis`)
        }
      }),
    )
  })

export async function sendControlCommand(
  baseUrl: string,
  token: string,
  command: ControlCommand,
): Promise<unknown> {
  const response = await fetch(`${baseUrl}/command`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(command),
  })
  const body = (await response.json()) as { ok?: boolean; result?: unknown; error?: string }
  if (!response.ok || body.ok !== true) {
    throw new Error(body.error ?? `command failed: HTTP ${response.status}`)
  }
  return body.result ?? null
}
