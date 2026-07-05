import { useEffect, useRef, useState } from "react"
import { decodeBootstrapResponse, decodeLiveFrame } from "@openagentsinc/khala-sync"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { KHALA_SYNC_DEMO_CLIENT_GROUP_ID } from "../config/khala-sync-demo"
import {
  applyDeltaFrameOfType,
  buildBootstrapRequestBody,
  buildBootstrapUrl,
  buildConnectUrl,
  entitiesOfType,
  type EntityDecoder
} from "./khala-sync-entities-core"

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

export type KhalaSyncCollectionStatus = "missing_token" | "loading" | "ready" | "error"

export type KhalaSyncCollectionState<T> = Readonly<{
  status: KhalaSyncCollectionStatus
  items: ReadonlyArray<T>
  error: string | null
}>

/**
 * Bootstraps one Khala Sync scope for entities of a single type, then live-
 * tails /api/sync/connect and merges each DeltaFrame in. Generic over the
 * decoded item shape so the same hook backs both the thread list
 * (chat_thread in scope.user.<owner>) and a thread's message list
 * (chat_message in scope.thread.<id>).
 */
export function useKhalaSyncCollection<T>(
  scope: string,
  entityType: string,
  decode: EntityDecoder<T>,
  idOf: (item: T) => string
): KhalaSyncCollectionState<T> {
  const { baseUrl, token } = useKhalaAuth()
  const [state, setState] = useState<KhalaSyncCollectionState<T>>({
    error: null,
    items: [],
    status: token === "" || scope === "" ? "missing_token" : "loading"
  })
  const itemsRef = useRef<ReadonlyArray<T>>([])

  useEffect(() => {
    if (token === "" || scope === "") return undefined
    let cancelled = false
    let socket: WebSocket | null = null
    itemsRef.current = []
    setState({ error: null, items: [], status: "loading" })

    const run = async () => {
      let cursor = 0
      try {
        const response = await fetch(buildBootstrapUrl(baseUrl), {
          body: JSON.stringify(buildBootstrapRequestBody(scope, KHALA_SYNC_DEMO_CLIENT_GROUP_ID)),
          headers: {
            authorization: `Bearer ${token}`,
            "content-type": "application/json"
          },
          method: "POST"
        })
        const body: unknown = await response.json()
        if (!response.ok) {
          const messageSafe =
            typeof body === "object" && body !== null && "messageSafe" in body
              ? String((body as { messageSafe: unknown }).messageSafe)
              : `bootstrap failed (${response.status})`
          throw new Error(messageSafe)
        }
        const decoded = decodeBootstrapResponse(body)
        cursor = decoded.cursor ?? 0
        const items = entitiesOfType(decoded.entities, entityType, decode)
        itemsRef.current = items
        if (!cancelled) setState({ error: null, items, status: "ready" })
      } catch (error) {
        if (!cancelled) {
          setState({ error: error instanceof Error ? error.message : String(error), items: [], status: "error" })
        }
        return
      }

      if (cancelled) return

      socket = new RNWebSocket(buildConnectUrl(baseUrl, scope, cursor), [], {
        headers: { authorization: `Bearer ${token}` }
      })
      socket.onmessage = event => {
        try {
          const frame = decodeLiveFrame(JSON.parse(String(event.data)))
          if (frame._tag !== "DeltaFrame") return
          const next = applyDeltaFrameOfType(itemsRef.current, frame, entityType, idOf, decode)
          itemsRef.current = next
          if (!cancelled) setState({ error: null, items: next, status: "ready" })
        } catch {
          // malformed/unrecognized frame — ignore rather than crash the feed
        }
      }
    }

    void run()

    return () => {
      cancelled = true
      socket?.close()
    }
  }, [scope, entityType, baseUrl, token])

  return state
}
