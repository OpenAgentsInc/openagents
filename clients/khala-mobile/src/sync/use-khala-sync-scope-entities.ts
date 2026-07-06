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

/** Pure phase -> UI-state mapping (KS session `ScopeSyncState["phase"]` is a
 * closed union; this only reads the string tag, so no import is needed).
 * `must_refetch` is a TERMINAL phase the session parks in after its own
 * bounded bootstrap retries are exhausted — it is NOT "loading" and NOT
 * "live", so without an explicit arm here a scope stuck there (0 items)
 * silently fell through to "loading" forever with no error ever shown. Real
 * bug: a brand-new user's first sign-in landed on an eternal "Loading
 * threads" spinner with no way to tell anything had gone wrong. */
export const resolveScopeEntitiesStatusAndError = (
  phase: string,
  itemCount: number,
): Readonly<{ status: KhalaSyncScopeEntitiesStatus; error: string | null }> => {
  if (phase === "denied") {
    return { error: "Khala Sync scope access was denied", status: "error" }
  }
  if (phase === "must_refetch") {
    return {
      error: "Khala Sync could not finish loading this scope. Restart the app to retry.",
      status: "error",
    }
  }
  if (itemCount > 0 || phase === "live") {
    return { error: null, status: "ready" }
  }
  return { error: null, status: "loading" }
}

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
    let retriedMustRefetch = false
    const syncScope = SyncScope.make(scope)

    const refresh = async () => {
      try {
        const entities = await runEffect(store.readEntities(syncScope, entityType))
        if (cancelled) return
        const items = decodeConfirmed(entities, decodeRef.current)
        const phase = session.state(syncScope).phase
        // One bounded, automatic recovery attempt: `must_refetch` means the
        // session's OWN bootstrap retries already ran out, but re-calling
        // subscribe() on a scope that isn't mid-loop restarts the loop from
        // scratch (session.ts resets `loopRunning` once the prior attempt's
        // driveScope settles) — a real, cheap self-heal for a transient
        // failure. Only ever tried once per mount so a persistently broken
        // scope still surfaces as an error instead of retrying forever.
        if (phase === "must_refetch" && !retriedMustRefetch) {
          retriedMustRefetch = true
          void runEffect(session.subscribe(syncScope)).catch(() => {
            // A rejection here just means the retry attempt itself failed
            // to even start; the phase-driven error state below still
            // surfaces to the user either way.
          })
        }
        const { error, status } = resolveScopeEntitiesStatusAndError(phase, items.length)
        setState({ error, items, status })
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
