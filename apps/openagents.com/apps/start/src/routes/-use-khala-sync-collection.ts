import { useEffect, useRef, useState } from 'react'
import { decodeBootstrapResponse, decodeLiveFrame } from '@openagentsinc/khala-sync'

import {
  applyDeltaFrameOfType,
  buildBootstrapRequestBody,
  buildConnectUrl,
  entitiesOfType,
  KHALA_SYNC_WEB_BOOTSTRAP_PATH,
  type EntityDecoder,
} from './-chat-sync-web-core'

/**
 * Bootstraps one Khala Sync scope for entities of a single type through the
 * local same-origin proxy (`../khala-sync-proxy.ts`), then live-tails the
 * proxied `/api/khala-sync/connect` WebSocket and merges each `DeltaFrame`
 * in. Ported from `clients/khala-mobile/src/sync/use-khala-sync-collection.ts`
 * (issue #8413) — the web version needs no header trick on the `WebSocket`
 * constructor because the proxy (not this hook) holds the bearer token and
 * attaches it upstream; the browser's own cookie carries the session to the
 * proxy, same-origin, automatically.
 */

export type KhalaSyncWebCollectionStatus = 'signed_out' | 'loading' | 'ready' | 'error'

export type KhalaSyncWebCollectionState<T> = Readonly<{
  status: KhalaSyncWebCollectionStatus
  items: ReadonlyArray<T>
  error: string | null
}>

const CLIENT_GROUP_ID = 'khala-web-start-chat-ui'

export function useKhalaSyncWebCollection<T>(
  input: Readonly<{
    signedIn: boolean
    scope: string
    entityType: string
    decode: EntityDecoder<T>
    idOf: (item: T) => string
  }>,
): KhalaSyncWebCollectionState<T> {
  const { decode, entityType, idOf, scope, signedIn } = input
  const [state, setState] = useState<KhalaSyncWebCollectionState<T>>({
    error: null,
    items: [],
    status: signedIn ? 'loading' : 'signed_out',
  })
  const itemsRef = useRef<ReadonlyArray<T>>([])

  useEffect(() => {
    if (!signedIn || scope === '') {
      setState({ error: null, items: [], status: 'signed_out' })
      return undefined
    }

    let cancelled = false
    let socket: WebSocket | null = null
    itemsRef.current = []
    setState({ error: null, items: [], status: 'loading' })

    const run = async () => {
      let cursor = 0
      try {
        const response = await fetch(KHALA_SYNC_WEB_BOOTSTRAP_PATH, {
          body: JSON.stringify(buildBootstrapRequestBody(scope, CLIENT_GROUP_ID)),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        })
        const body: unknown = await response.json()
        if (!response.ok) {
          const messageSafe =
            typeof body === 'object' && body !== null && 'messageSafe' in body
              ? String((body as { messageSafe: unknown }).messageSafe)
              : `bootstrap failed (${response.status})`
          throw new Error(messageSafe)
        }
        const decoded = decodeBootstrapResponse(body)
        cursor = decoded.cursor ?? 0
        const items = entitiesOfType(decoded.entities, entityType, decode)
        itemsRef.current = items
        if (!cancelled) setState({ error: null, items, status: 'ready' })
      } catch (error) {
        if (!cancelled) {
          setState({
            error: error instanceof Error ? error.message : String(error),
            items: [],
            status: 'error',
          })
        }
        return
      }

      if (cancelled) return

      socket = new WebSocket(buildConnectUrl(scope, cursor, window.location))
      socket.onmessage = event => {
        try {
          const frame = decodeLiveFrame(JSON.parse(String(event.data)))
          if (frame._tag !== 'DeltaFrame') return
          const next = applyDeltaFrameOfType(itemsRef.current, frame, entityType, idOf, decode)
          itemsRef.current = next
          if (!cancelled) setState({ error: null, items: next, status: 'ready' })
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
  }, [decode, entityType, idOf, scope, signedIn])

  return state
}
