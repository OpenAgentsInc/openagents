import { randomUUID } from "expo-crypto"
import { openDatabaseSync } from "expo-sqlite"

import {
  openExpoKhalaSyncStore,
  type ExpoSqliteDatabase,
} from "@openagentsinc/khala-sync-client/expo-sqlite-store"
import { loadNativeSessionCredential } from "../auth/native-session-vault"
import {
  fetchMobileExecutionTargetCatalog,
  type MobileExecutionTargetCatalog,
} from "../coding/mobile-execution-targets"
import { openMobileSyncHostCore, type MobileSyncHost } from "./mobile-sync-host-core"

export type MobileNativeSyncHost = MobileSyncHost & Readonly<{
  connectStoredVerifiedSession: () => Promise<"connected" | "signed_out" | "unavailable">
  /** Public-safe target projection. Credential custody never leaves this host. */
  executionTargets: () => Promise<MobileExecutionTargetCatalog | null>
}>

export const OPENAGENTS_MOBILE_SYNC_DATABASE = "openagents-mobile-sync.sqlite"
export const OPENAGENTS_MOBILE_SYNC_BASE_URL = "https://openagents.com"

const openNativeStore = (databaseName: string) =>
  openExpoKhalaSyncStore(databaseName, name => {
    const database = openDatabaseSync(name)
    const adapter: ExpoSqliteDatabase = {
      execSync: sql => database.execSync(sql),
      runSync: (sql, ...params) => database.runSync(sql, ...params),
      getAllSync: <Row>(sql: string, ...params: ReadonlyArray<string | number>) =>
        database.getAllSync<Row>(sql, ...params),
      withTransactionSync: task => database.withTransactionSync(task),
      closeSync: () => database.closeSync(),
    }
    return adapter
  })

/** Open one host-owned local store and a host-only verified-session connector. */
export const openMobileSyncHost = (): MobileNativeSyncHost => {
  const host = openMobileSyncHostCore({
    databaseName: OPENAGENTS_MOBILE_SYNC_DATABASE,
    randomId: randomUUID,
    openStore: openNativeStore,
  })
  return {
    ...host,
    executionTargets: async () => {
      try {
        const credential = await loadNativeSessionCredential()
        if (credential === null || host.conversation() === null) return null
        return await fetchMobileExecutionTargetCatalog({
          baseUrl: OPENAGENTS_MOBILE_SYNC_BASE_URL,
          token: credential.accessToken,
        })
      } catch {
        return null
      }
    },
    connectStoredVerifiedSession: async () => {
      try {
        const credential = await loadNativeSessionCredential()
        if (credential === null) {
          host.disconnectAuthenticated()
          return "signed_out"
        }
        host.connectAuthenticated({
          verification:"server_verified",
          baseUrl: OPENAGENTS_MOBILE_SYNC_BASE_URL,
          ownerUserId: credential.ownerUserId,
          authToken: () => credential.accessToken,
        })
        return "connected"
      } catch {
        host.disconnectAuthenticated()
        return "unavailable"
      }
    },
  }
}
