import { useEffect, useRef, useState } from "react"
import { ScrollView, Text, View } from "react-native"

import {
  KHALA_SYNC_DEMO_BASE_URL,
  KHALA_SYNC_DEMO_CLIENT_GROUP_ID,
  KHALA_SYNC_DEMO_THREAD_ID,
  KHALA_SYNC_DEMO_TOKEN
} from "../config/khala-sync-demo"
import {
  buildBootstrapRequestBody,
  buildBootstrapUrl,
  buildConnectUrl,
  chatFeedScope,
  makeFeedEvent,
  type ChatFeedEvent
} from "./khala-chat-feed-core"

const MAX_EVENTS = 200

/** React Native's WebSocket accepts a third `{ headers }` arg for the
 * upgrade request (a RN-specific extension beyond the DOM WebSocket type
 * this repo's TypeScript lib picks up), so this constructor is typed by
 * hand rather than fighting the ambient DOM declaration. */
type RNWebSocketConstructor = new (
  url: string,
  protocols: ReadonlyArray<string>,
  options: { headers: Record<string, string> }
) => WebSocket
const RNWebSocket = WebSocket as unknown as RNWebSocketConstructor

export const KhalaChatFeed = () => {
  const [events, setEvents] = useState<ReadonlyArray<ChatFeedEvent>>([])
  const seqRef = useRef(0)

  const pushEvent = (kind: ChatFeedEvent["kind"], payload: unknown) => {
    seqRef.current += 1
    const event = makeFeedEvent(kind, payload, new Date().toISOString(), seqRef.current)
    setEvents(previous => [event, ...previous].slice(0, MAX_EVENTS))
  }

  useEffect(() => {
    if (KHALA_SYNC_DEMO_TOKEN === "") {
      pushEvent("error", {
        error: "missing_demo_token",
        hint: "export EXPO_PUBLIC_KHALA_SYNC_DEMO_TOKEN before starting the app"
      })
      return
    }

    let cancelled = false
    let socket: WebSocket | null = null
    const scope = chatFeedScope(KHALA_SYNC_DEMO_THREAD_ID)

    const run = async () => {
      let cursor = 0
      try {
        const response = await fetch(buildBootstrapUrl(KHALA_SYNC_DEMO_BASE_URL), {
          body: JSON.stringify(
            buildBootstrapRequestBody(scope, KHALA_SYNC_DEMO_CLIENT_GROUP_ID)
          ),
          headers: {
            authorization: `Bearer ${KHALA_SYNC_DEMO_TOKEN}`,
            "content-type": "application/json"
          },
          method: "POST"
        })
        const body: unknown = await response.json()
        if (cancelled) return
        pushEvent("bootstrap", body)
        if (
          response.ok &&
          typeof body === "object" &&
          body !== null &&
          "cursor" in body &&
          typeof (body as { cursor?: unknown }).cursor === "number"
        ) {
          cursor = (body as { cursor: number }).cursor
        }
      } catch (error) {
        if (!cancelled) {
          pushEvent("error", { error: String(error) })
        }
        return
      }

      if (cancelled) return

      socket = new RNWebSocket(
        buildConnectUrl(KHALA_SYNC_DEMO_BASE_URL, scope, cursor),
        [],
        { headers: { authorization: `Bearer ${KHALA_SYNC_DEMO_TOKEN}` } }
      )
      socket.onmessage = event => {
        try {
          pushEvent("frame", JSON.parse(String(event.data)))
        } catch {
          pushEvent("frame", { raw: String(event.data) })
        }
      }
      socket.onerror = () => {
        if (!cancelled) pushEvent("error", { error: "websocket_error" })
      }
      socket.onclose = closeEvent => {
        if (!cancelled) {
          pushEvent("error", { code: closeEvent.code, reason: closeEvent.reason })
        }
      }
    }

    void run()

    return () => {
      cancelled = true
      socket?.close()
    }
  }, [])

  return (
    <View className="mt-8 w-full flex-1 gap-2 px-4">
      <Text className="font-sans text-sm text-textMuted">
        chat feed — {KHALA_SYNC_DEMO_THREAD_ID}
      </Text>
      <ScrollView className="flex-1 rounded-lg border border-border bg-surface">
        {events.length === 0 ? (
          <Text className="p-3 font-mono text-xs text-textFaint">
            waiting for events…
          </Text>
        ) : (
          events.map(event => (
            <View key={event.id} className="border-b border-borderMuted p-3">
              <Text className="font-mono text-xs text-textFaint">
                {event.kind} · {event.receivedAt}
              </Text>
              <Text className="mt-1 font-mono text-xs text-text">{event.raw}</Text>
            </View>
          ))
        )}
      </ScrollView>
    </View>
  )
}
