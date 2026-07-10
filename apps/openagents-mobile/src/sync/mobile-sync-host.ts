import { randomUUID } from "expo-crypto"
import { openDatabaseSync } from "expo-sqlite"

import {
  openExpoKhalaSyncStore,
  type ExpoSqliteDatabase,
} from "@openagentsinc/khala-sync-client/expo-sqlite-store"
import { openMobileSyncHostCore, type MobileSyncHost } from "./mobile-sync-host-core"

export type { MobileSyncHost } from "./mobile-sync-host-core"

export const OPENAGENTS_MOBILE_SYNC_DATABASE = "openagents-mobile-sync.sqlite"

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

/** Open one host-owned local store; authenticated network Sync is separate. */
export const openMobileSyncHost = (): MobileSyncHost =>
  openMobileSyncHostCore({
    databaseName: OPENAGENTS_MOBILE_SYNC_DATABASE,
    randomId: randomUUID,
    openStore: openNativeStore,
  })
