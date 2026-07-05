import { useCallback, useMemo, useRef } from 'react'
import { decodePushResponse, type PushResponse } from '@openagentsinc/khala-sync'

import {
  buildPushRequestBody,
  KHALA_SYNC_WEB_PUSH_PATH,
  makeSafeRef,
  stableArgsJson,
} from './-chat-sync-web-core'

export type PendingMutation = Readonly<{
  name: string
  args: Record<string, unknown>
}>

/**
 * POSTs a batch of mutations to the local same-origin `/api/khala-sync/push`
 * proxy and returns the decoded response — ported from
 * `clients/khala-mobile/src/sync/use-khala-sync-push.ts` (issue #8413).
 * Generates a fresh clientId per page load (not persisted across reloads) so
 * the mutationId counter can always start at 1 without colliding with a
 * stale server-side ledger watermark from a prior session.
 */
export function useKhalaSyncWebPush(): (
  mutations: ReadonlyArray<PendingMutation>,
) => Promise<PushResponse> {
  const clientIdRef = useRef<string | undefined>(undefined)
  if (clientIdRef.current === undefined) clientIdRef.current = makeSafeRef('web-composer')
  const nextMutationIdRef = useRef(1)
  const clientGroupId = useMemo(() => 'khala-web-start-composer', [])

  return useCallback(
    async (mutations: ReadonlyArray<PendingMutation>): Promise<PushResponse> => {
      const envelopes = mutations.map(mutation => ({
        argsJson: stableArgsJson(mutation.args),
        mutationId: nextMutationIdRef.current++,
        name: mutation.name,
      }))
      const body = buildPushRequestBody({
        clientGroupId,
        clientId: clientIdRef.current as string,
        mutations: envelopes,
      })
      const response = await fetch(KHALA_SYNC_WEB_PUSH_PATH, {
        body: JSON.stringify(body),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      })
      const json: unknown = await response.json()
      if (!response.ok) {
        const messageSafe =
          typeof json === 'object' && json !== null && 'messageSafe' in json
            ? String((json as { messageSafe: unknown }).messageSafe)
            : `push failed (${response.status})`
        throw new Error(messageSafe)
      }
      const decoded = decodePushResponse(json)
      const rejected = decoded.results.find(result => result.status === 'rejected')
      if (rejected !== undefined) {
        throw new Error(rejected.errorMessageSafe ?? rejected.errorCode ?? 'mutation rejected')
      }
      return decoded
    },
    [clientGroupId],
  )
}
