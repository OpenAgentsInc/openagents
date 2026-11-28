// Firm reason: 1 (Effect UI Query.StreamService SSE client for browser DOM/fetch in docs/effect/half-day-sprint.md)
import { Query } from "@effect/ui"
import { Effect, Queue, Schema, Stream } from "effect"
import { SessionSnapshot } from "../../shared/wire.ts"

export const SessionStreamError = Schema.TaggedStruct("SessionStreamError", {
  message: Schema.String
})
export type SessionStreamError = typeof SessionStreamError.Type

const streamError = (message: string): SessionStreamError => ({
  _tag: "SessionStreamError",
  message
})

export class SessionStream extends Query.StreamService<SessionStream>()(
  "oa-spike/SessionStream",
  {
    args: { sessionId: Schema.String },
    success: SessionSnapshot,
    error: SessionStreamError
  }
) {
  static readonly layerBrowser = this.toLayer(({ sessionId }) =>
    Stream.callback<SessionSnapshot, SessionStreamError>((queue) =>
      Effect.gen(function*() {
        const controller = new AbortController()
        yield* Effect.addFinalizer(() =>
          Effect.sync(() => {
            controller.abort()
          })
        )

        const url = `/api/sessions/${encodeURIComponent(sessionId)}/stream`
        const pump = async () => {
          try {
            const response = await fetch(url, {
              headers: { accept: "text/event-stream" },
              signal: controller.signal
            })
            if (!response.ok || !response.body) {
              await Effect.runPromise(
                Queue.fail(
                  queue,
                  streamError(`Stream failed (${response.status})`)
                )
              )
              return
            }
            const reader = response.body.getReader()
            const decoder = new TextDecoder()
            let buffer = ""
            while (!controller.signal.aborted) {
              const { done, value } = await reader.read()
              if (done) break
              buffer += decoder.decode(value, { stream: true })
              let boundary = buffer.indexOf("\n\n")
              while (boundary !== -1) {
                const rawFrame = buffer.slice(0, boundary)
                buffer = buffer.slice(boundary + 2)
                const dataLine = rawFrame
                  .split("\n")
                  .find((line) => line.startsWith("data: "))
                if (dataLine) {
                  try {
                    const parsed = JSON.parse(dataLine.slice(6)) as SessionSnapshot
                    Queue.offerUnsafe(queue, parsed)
                  } catch {
                    // ignore malformed keepalive or partial frame
                  }
                }
                boundary = buffer.indexOf("\n\n")
              }
            }
            await Effect.runPromise(Queue.end(queue))
          } catch (err) {
            if (controller.signal.aborted) {
              await Effect.runPromise(Queue.end(queue))
              return
            }
            await Effect.runPromise(
              Queue.fail(
                queue,
                streamError(err instanceof Error ? err.message : String(err))
              )
            )
          }
        }

        void pump()
      })
    )
  )

  static readonly layerSsr = this.toLayer(({ sessionId }) =>
    Stream.succeed<SessionSnapshot>({
      sessionId,
      seq: 0,
      status: "starting",
      activeTurn: 0,
      title: "",
      cwd: "",
      model: "google/gemini-4-neon",
      messages: [],
      pendingPermit: null,
      todos: [],
      stats: null,
      lastError: null,
      workerPid: null
    })
  )
}
