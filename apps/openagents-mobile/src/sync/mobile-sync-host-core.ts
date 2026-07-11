import {
  ClientGroupId,
  ClientId,
  personalScope,
  SyncSchemaVersion,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import {
  createHttpKhalaSyncTransport,
  createKhalaSyncSession,
  createOverlay,
  type HttpTransportConfig,
  type KhalaSyncSession,
  type KhalaSyncSessionOptions,
  type KhalaSyncTransport,
  type ScopeSyncState,
} from "@openagentsinc/khala-sync-client"
import type { KhalaSyncExpoSqliteStore } from "@openagentsinc/khala-sync-client/expo-sqlite-store"
import { Effect } from "effect"

export const MobileSyncSchemaVersion = SyncSchemaVersion.make(1)

export type MobileSyncHostStatus = Readonly<{
  state: "local_ready" | "closed"
  syncPhase: ScopeSyncState["phase"] | "closed"
  lastDeltaAt: number | null
  schemaVersion: number
  identityState: "persisted"
  pendingMutationCount: number
}>

export type MobileSyncHost = Readonly<{
  status: () => MobileSyncHostStatus
  connectAuthenticated: (input: MobileAuthenticatedSyncInput) => void
  disconnectAuthenticated: () => void
  close: () => void
}>

export type MobileAuthenticatedSyncInput = Readonly<{
  baseUrl: string
  ownerUserId: string
  authToken: () => string
  createTransport?: (config: HttpTransportConfig) => KhalaSyncTransport
  sessionOptions?: KhalaSyncSessionOptions
}>

/**
 * Host lifecycle around the shared Expo SQLite adapter. This core accepts its
 * native opener so Bun can prove restart semantics without loading Expo.
 */
export const openMobileSyncHostCore = (input: Readonly<{
  databaseName: string
  randomId: () => string
  openStore: (databaseName: string) => KhalaSyncExpoSqliteStore
}>): MobileSyncHost => {
  const store = input.openStore(input.databaseName)
  let closed = false
  let session: KhalaSyncSession | null = null
  let scope: SyncScope | null = null
  try {
    const persisted = Effect.runSync(store.identity())
    if (persisted === null) {
      Effect.runSync(store.setIdentity({
        clientGroupId: ClientGroupId.make(`openagents-mobile.${input.randomId()}`),
        clientId: ClientId.make(`mobile.${input.randomId()}`),
        schemaVersion: MobileSyncSchemaVersion,
      }))
    }
  } catch (error) {
    try {
      Effect.runSync(store.close())
    } catch {
      // Preserve the identity/bootstrap failure as the actionable error.
    }
    throw error
  }

  const disconnectAuthenticated = (): void => {
    if (session === null) return
    Effect.runSync(session.close())
    session = null
    scope = null
  }

  return {
    status: () => ({
      state: closed ? "closed" : "local_ready",
      syncPhase: closed
        ? "closed"
        : session === null || scope === null
          ? "idle"
          : session.state(scope).phase,
      lastDeltaAt: closed || session === null || scope === null
        ? null
        : session.lastDeltaAt(scope),
      schemaVersion: Number(MobileSyncSchemaVersion),
      identityState: "persisted",
      pendingMutationCount: closed ? 0 : Effect.runSync(store.pendingMutations()).length,
    }),
    connectAuthenticated: connection => {
      if (closed) throw new Error("mobile Sync host is closed")
      const ownerUserId = connection.ownerUserId.trim()
      if (ownerUserId === "" || connection.authToken().trim() === "") {
        throw new Error("mobile authenticated Sync credential is incomplete")
      }
      disconnectAuthenticated()
      const identity = Effect.runSync(store.identity())
      if (identity === null) throw new Error("mobile Sync identity is unavailable")
      const overlay = Effect.runSync(createOverlay(store, []))
      const transportConfig = {
        baseUrl: connection.baseUrl,
        authToken: connection.authToken,
      }
      const transport = connection.createTransport?.(transportConfig) ??
        createHttpKhalaSyncTransport(transportConfig)
      scope = personalScope(ownerUserId)
      session = createKhalaSyncSession({
        baseUrl: connection.baseUrl,
        clientGroupId: identity.clientGroupId,
        clientId: identity.clientId,
        schemaVersion: identity.schemaVersion,
        authToken: connection.authToken,
      }, store, overlay, transport, connection.sessionOptions)
      Effect.runSync(session.subscribe(scope))
    },
    disconnectAuthenticated,
    close: () => {
      if (closed) return
      closed = true
      disconnectAuthenticated()
      Effect.runSync(store.close())
    },
  }
}
