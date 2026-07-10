import { chmodSync, mkdirSync } from "node:fs"
import path from "node:path"

import { ClientGroupId, ClientId, SyncSchemaVersion } from "@openagentsinc/khala-sync"
import { Effect } from "effect"
import { openDesktopSyncStore, type DesktopSyncStore } from "./desktop-sync-store.ts"

export const DesktopSyncSchemaVersion = SyncSchemaVersion.make(1)

export type DesktopSyncHostStatus = Readonly<{
  state: "local_ready" | "closed"
  schemaVersion: number
  identityState: "persisted"
  pendingMutationCount: number
}>

export type DesktopSyncHost = Readonly<{
  status: () => DesktopSyncHostStatus
  close: () => void
}>

const secureLocalFiles = (databasePath: string): void => {
  if (process.platform === "win32") return
  for (const candidate of [databasePath, `${databasePath}-wal`, `${databasePath}-shm`]) {
    try {
      chmodSync(candidate, 0o600)
    } catch {
      // WAL/SHM are created lazily. The owning directory remains mode 0700.
    }
  }
}

export const openDesktopSyncHost = (input: Readonly<{
  databasePath: string
  randomId: () => string
  openStore?: (path: string) => DesktopSyncStore
}>): DesktopSyncHost => {
  const directory = path.dirname(input.databasePath)
  mkdirSync(directory, { recursive: true, mode: 0o700 })
  if (process.platform !== "win32") chmodSync(directory, 0o700)

  const store = (input.openStore ?? openDesktopSyncStore)(input.databasePath)
  let closed = false
  try {
    const persisted = Effect.runSync(store.identity())
    if (persisted === null) {
      Effect.runSync(store.setIdentity({
        clientGroupId: ClientGroupId.make(`openagents-desktop.${input.randomId()}`),
        clientId: ClientId.make(`desktop.${input.randomId()}`),
        schemaVersion: DesktopSyncSchemaVersion,
      }))
    }
    secureLocalFiles(input.databasePath)
  } catch (error) {
    Effect.runSync(store.close())
    throw error
  }

  return {
    status: () => ({
      state: closed ? "closed" : "local_ready",
      schemaVersion: Number(DesktopSyncSchemaVersion),
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
