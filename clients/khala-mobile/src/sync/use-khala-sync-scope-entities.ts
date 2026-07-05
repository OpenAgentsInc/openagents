import { useEffect, useRef, useState } from "react"
import { SyncScope } from "@openagentsinc/khala-sync"
import type { ConfirmedEntity, KhalaSyncLocalStore, KhalaSyncOverlay, KhalaSyncSession } from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"

/**
 * Local-first, delta-synced read of one Khala Sync scope/entity-type pair,
 * backed directly by the already-durable Expo SQLite store + overlay +
 * session primitives opened once per app session
 * (`khala-mobile-sync-runtime.ts`).
 *
 * This is the fix for "every time i open a new thread session in the app it
 * loads the messages from scratch": on mount it reads whatever confirmed
 * rows are ALREADY on-device for this scope (near-instant SQLite read, no
 * network wait) and renders them immediately, while `session.subscribe`
 * kicks off the durable-cursor sync loop
 * (`packages/khala-sync-client/src/session.ts` `driveScope`) in the
 * background. That loop only re-bootstraps a scope with NO durable cursor
 * yet; once one exists (this scope was ever visited before), it resumes
 * with a bounded catch-up of just the entries committed since that cursor,
 * then live-tails — never a full history re-fetch on a revisit.
 *
 * Confirmed rows land in the SQLite store via `overlay.onConfirmed`
 * (bootstrap snapshot, catch-up page, or live delta), which this hook
 * observes through `overlay.subscribe` + `session.subscribeState` and
 * re-reads from the store on each notification — the same read path
 * `khala-mobile-sync-runtime.ts`'s `confirmedChatMessages`/
 * `confirmedChatThreads` already use for one-shot snapshots, made live here.
 */
export type KhalaSyncScopeEntitiesStatus = "loading" | "ready" | "error"

export type KhalaSyncScopeEntitiesState<T> = Readonly<{
  status: KhalaSyncScopeEntitiesStatus
  items: ReadonlyArray<T>
  error: string | null
}>

export type KhalaSyncScopeEntitiesInput<T> = Readonly<{
  decode: (value: unknown) => T
  entityType: string
  overlay: KhalaSyncOverlay | null
  scope: string
  session: KhalaSyncSession | null
  store: KhalaSyncLocalStore | null
}>

const runEffect = <A, E>(effect: Effect.Effect<A, E>): Promise<A> => Effect.runPromise(effect)

const decodeConfirmed = <T>(
  entities: ReadonlyArray<ConfirmedEntity>,
  decode: (value: unknown) => T
): ReadonlyArray<T> =>
  entities.map(entity => decode(JSON.parse(entity.postImageJson) as unknown))

export function useKhalaSyncScopeEntities<T>(
  input: KhalaSyncScopeEntitiesInput<T>
): KhalaSyncScopeEntitiesState<T> {
  const { decode, entityType, overlay, scope, session, store } = input
  const [state, setState] = useState<KhalaSyncScopeEntitiesState<T>>({
    error: null,
    items: [],
    status: "loading"
  })
  const decodeRef = useRef(decode)
  decodeRef.current = decode

  useEffect(() => {
    if (session === null || overlay === null || store === null || scope === "") {
      setState({ error: null, items: [], status: "loading" })
      return undefined
    }
    let cancelled = false
    const syncScope = SyncScope.make(scope)

    const refresh = async () => {
      try {
        const entities = await runEffect(store.readEntities(syncScope, entityType))
        if (cancelled) return
        const items = decodeConfirmed(entities, decodeRef.current)
        const phase = session.state(syncScope).phase
        const status: KhalaSyncScopeEntitiesStatus =
          phase === "denied"
            ? "error"
            : items.length > 0 || phase === "live"
              ? "ready"
              : "loading"
        setState({
          error: phase === "denied" ? "Khala Sync scope access was denied" : null,
          items,
          status
        })
      } catch (error) {
        if (cancelled) return
        setState({
          error: error instanceof Error ? error.message : String(error),
          items: [],
          status: "error"
        })
      }
    }

    void refresh()
    void runEffect(session.subscribe(syncScope)).catch(error => {
      if (cancelled) return
      setState({
        error: error instanceof Error ? error.message : String(error),
        items: [],
        status: "error"
      })
    })

    const unsubscribeState = session.subscribeState(changedScope => {
      if (changedScope !== syncScope) return
      void refresh()
    })
    const unsubscribeOverlay = overlay.subscribe(changedScope => {
      if (changedScope !== syncScope) return
      void refresh()
    })

    return () => {
      cancelled = true
      unsubscribeState()
      unsubscribeOverlay()
      void runEffect(session.unsubscribe(syncScope))
    }
  }, [session, overlay, store, scope, entityType])

  return state
}
