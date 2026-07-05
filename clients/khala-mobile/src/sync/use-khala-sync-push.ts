import { useCallback, useMemo, useRef } from "react"
import { decodePushResponse, type PushResponse } from "@openagentsinc/khala-sync"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { readOkMobileJsonResponse } from "../network/mobile-problem"
import { buildPushRequestBody, buildPushUrl, makeSafeRef, stableArgsJson } from "./khala-sync-push-core"

export type PendingMutation = Readonly<{
  name: string
  args: Record<string, unknown>
}>

/** POSTs a batch of mutations to /api/sync/push and returns the decoded
 * response. Generates a fresh clientId per app session (not reused across
 * relaunches) so the mutationId counter can always start at 1 without
 * colliding with a stale server-side ledger watermark from a prior session.
 *
 * `clientGroupId` MUST be unique per signed-in owner: `khala_sync_client_state`
 * permanently binds one client group to one user server-side and rejects
 * every push from a second user with the same group id as
 * `unauthorized_scope` ("This client group is bound to a different user") —
 * a client group can never migrate between users (see
 * `packages/khala-sync-server/src/mutation-ledger.ts`). This hook used to
 * hard-code the literal string `"khala-mobile-composer"` for every install
 * regardless of who was signed in, which meant only the FIRST Khala Sync
 * user ever to push from any khala-mobile install could ever push again —
 * every other real user's app would permanently fail every mobile-initiated
 * turn. Desktop already avoids this (`khala-code-desktop.<uuid>`, persisted
 * per install); mobile now derives a stable id from the signed-in owner
 * instead, so the same user always reuses the same bound group across
 * relaunches, and two different owners never collide. */
export function useKhalaSyncPush(): (mutations: ReadonlyArray<PendingMutation>) => Promise<PushResponse> {
  const { baseUrl, ownerUserId, token } = useKhalaAuth()
  const clientIdRef = useRef<string | undefined>(undefined)
  if (clientIdRef.current === undefined) clientIdRef.current = makeSafeRef("mobile-composer")
  const nextMutationIdRef = useRef(1)
  const clientGroupId = useMemo(
    () => `khala-mobile-composer.${ownerUserId || "unauthenticated"}`,
    [ownerUserId]
  )

  return useCallback(
    async (mutations: ReadonlyArray<PendingMutation>): Promise<PushResponse> => {
      const envelopes = mutations.map(mutation => ({
        argsJson: stableArgsJson(mutation.args),
        mutationId: nextMutationIdRef.current++,
        name: mutation.name
      }))
      const body = buildPushRequestBody({
        clientGroupId,
        clientId: clientIdRef.current as string,
        mutations: envelopes
      })
      const response = await fetch(buildPushUrl(baseUrl), {
        body: JSON.stringify(body),
        headers: {
          authorization: `Bearer ${token}`,
          "content-type": "application/json"
        },
        method: "POST"
      })
      const json = await readOkMobileJsonResponse(response, "sync push")
      const decoded = decodePushResponse(json)
      const rejected = decoded.results.find(result => result.status === "rejected")
      if (rejected !== undefined) {
        throw new Error(rejected.errorMessageSafe ?? rejected.errorCode ?? "mutation rejected")
      }
      return decoded
    },
    [clientGroupId, baseUrl, token]
  )
}
