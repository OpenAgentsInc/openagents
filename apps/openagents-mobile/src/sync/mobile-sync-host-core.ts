import { ClientGroupId, ClientId, SyncSchemaVersion } from "@openagentsinc/khala-sync"
import type { KhalaSyncExpoSqliteStore } from "@openagentsinc/khala-sync-client/expo-sqlite-store"
import { Effect } from "effect"

export const MobileSyncSchemaVersion = SyncSchemaVersion.make(1)

export type MobileSyncHostStatus = Readonly<{
  state: "local_ready" | "closed"
  schemaVersion: number
  identityState: "persisted"
  pendingMutationCount: number
}>

export type MobileSyncHost = Readonly<{
  status: () => MobileSyncHostStatus
  close: () => void
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

  return {
    status: () => ({
      state: closed ? "closed" : "local_ready",
      schemaVersion: Number(MobileSyncSchemaVersion),
      identityState: "persisted",
      pendingMutationCount: closed ? 0 : Effect.runSync(store.pendingMutations()).length,
    }),
    close: () => {
      if (closed) return
      closed = true
      Effect.runSync(store.close())
    },
  }
}
