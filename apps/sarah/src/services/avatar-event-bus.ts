/**
 * In-memory event bus for the Sarah avatar surface (#8598 AV-3).
 *
 * The brain endpoint and tool paths publish typed, public-safe card/transcript
 * events keyed by conversation_ref; the browser subscribes over SSE and pops
 * components as Sarah speaks. Single-instance scope is acceptable: the
 * publisher and subscriber both live in this service's process, and events are
 * ephemeral UI signals — the session index remains the durable record.
 */

export type SarahAvatarEvent = {
  type:
    | "transcript"
    | "card"
    | "guard_refusal"
    | "session"
  role?: "user" | "assistant"
  text?: string
  title?: string
  body?: string
  href?: string
  state?: string
  at: string
}

type Subscriber = (event: SarahAvatarEvent) => void

const subscribers = new Map<string, Set<Subscriber>>()

export function publishSarahAvatarEvent(
  conversationRef: string,
  event: Omit<SarahAvatarEvent, "at">,
): void {
  const full: SarahAvatarEvent = { ...event, at: new Date().toISOString() }
  for (const notify of subscribers.get(conversationRef) ?? []) {
    try {
      notify(full)
    } catch {
      // A broken subscriber never blocks the turn.
    }
  }
}

export function sarahAvatarEventStream(conversationRef: string): Response {
  let cleanup = () => {}
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      const encoder = new TextEncoder()
      const notify: Subscriber = (event) => {
        controller.enqueue(encoder.encode(`data: ${JSON.stringify(event)}\n\n`))
      }
      const set = subscribers.get(conversationRef) ?? new Set()
      set.add(notify)
      subscribers.set(conversationRef, set)
      controller.enqueue(encoder.encode(`: connected ${conversationRef}\n\n`))
      const heartbeat = setInterval(() => {
        try {
          controller.enqueue(encoder.encode(`: hb\n\n`))
        } catch {
          cleanup()
        }
      }, 25_000)
      cleanup = () => {
        clearInterval(heartbeat)
        set.delete(notify)
        if (set.size === 0) subscribers.delete(conversationRef)
      }
    },
    cancel() {
      cleanup()
    },
  })
  return new Response(stream, {
    headers: {
      "content-type": "text/event-stream",
      "cache-control": "no-cache",
      connection: "keep-alive",
    },
  })
}
