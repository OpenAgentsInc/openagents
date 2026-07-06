import { createHttpKhalaSyncTransport } from '@openagentsinc/khala-sync-client'
import { SyncVersionWatermark } from '@openagentsinc/khala-sync'
import { Effect } from 'effect'
import { useEffect, useRef, useState } from 'react'

import {
  applyTokensServedDelta,
  buildTokensServedBootstrapRequest,
  extractTokensServedSnapshot,
  TOKENS_SERVED_SCOPE,
  type TokensServedSnapshot,
} from './tokens-served-sync'

export type TokensServedLiveStatus = 'connecting' | 'live' | 'error'

export type TokensServedLiveState = Readonly<{
  status: TokensServedLiveStatus
  snapshot: TokensServedSnapshot | undefined
  errorMessage: string | undefined
}>

/**
 * Subscribes the dashboard to the PUBLIC `scope.public.tokens-served`
 * counter through Aiur's own same-origin Khala Sync proxy
 * (`../khala-sync-proxy.ts`) — real bootstrap + real live WebSocket tail
 * against production Khala Sync, proxied server-side with the signed-in
 * owner's bearer. This is AIUR-1's "at least one page renders a real live
 * Khala Sync scope" proof (#8499).
 */
export const useTokensServedLive = (): TokensServedLiveState => {
  const [state, setState] = useState<TokensServedLiveState>({
    status: 'connecting',
    snapshot: undefined,
    errorMessage: undefined,
  })
  const snapshotRef = useRef<TokensServedSnapshot | undefined>(undefined)

  useEffect(() => {
    let cancelled = false
    let liveSocketClose: (() => void) | undefined

    const run = async () => {
      const transport = createHttpKhalaSyncTransport({
        baseUrl: window.location.origin,
        // Ignored by Aiur's proxy (it authenticates from the owner's own
        // httpOnly session cookie, never a client-supplied value) — kept
        // non-empty only because the transport always sends a header/query
        // value.
        authToken: () => 'aiur-proxy-authenticates-via-session-cookie',
      })

      const bootstrap = await Effect.runPromise(
        transport.bootstrap(
          buildTokensServedBootstrapRequest('aiur-dashboard'),
        ),
      ).catch((error: unknown) => {
        return { _error: error } as const
      })

      if (cancelled) return

      if ('_error' in bootstrap) {
        setState({
          status: 'error',
          snapshot: undefined,
          errorMessage:
            bootstrap._error instanceof Error
              ? bootstrap._error.message
              : 'Khala Sync bootstrap failed.',
        })
        return
      }

      const snapshot = extractTokensServedSnapshot(bootstrap.entities)
      snapshotRef.current = snapshot
      setState({ status: 'live', snapshot, errorMessage: undefined })

      const cursor = bootstrap.cursor ?? SyncVersionWatermark.make(0)

      const liveSocket = await Effect.runPromise(
        transport.connectLive(TOKENS_SERVED_SCOPE, cursor, {
          onFrame: frame => {
            if (frame._tag !== 'DeltaFrame') return
            const next = applyTokensServedDelta(snapshotRef.current, frame)
            if (next === snapshotRef.current) return
            snapshotRef.current = next
            setState({ status: 'live', snapshot: next, errorMessage: undefined })
          },
          onClose: cause => {
            if (cancelled) return
            setState(prev => ({
              status: cause.error === undefined ? prev.status : 'error',
              snapshot: prev.snapshot,
              errorMessage: cause.error?.message,
            }))
          },
        }),
      ).catch(() => undefined)

      if (cancelled) {
        liveSocket?.close()
        return
      }

      liveSocketClose = liveSocket?.close
    }

    void run()

    return () => {
      cancelled = true
      liveSocketClose?.()
    }
  }, [])

  return state
}
