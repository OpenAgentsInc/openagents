import { describe, expect, test } from "bun:test"
import { mkdtempSync, rmSync, statSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { Database } from "bun:sqlite"

import { Effect } from "effect"

import { openDesktopSyncHost } from "../src/desktop-sync-host.ts"
import {
  openDesktopSyncStore,
  type DesktopSqliteDatabase,
  type DesktopSyncStore,
} from "../src/desktop-sync-store.ts"

const openBunDatabase = (databasePath: string): DesktopSqliteDatabase => {
  const database = new Database(databasePath, { create: true })
  return {
    exec: sql => database.exec(sql),
    prepare: sql => {
      const statement = database.query(sql)
      return {
        run: (...params) => statement.run(...params),
        all: (...params) => statement.all(...params),
      }
    },
    close: () => database.close(),
  }
}

const openTestStore = (databasePath: string): DesktopSyncStore =>
  openDesktopSyncStore(databasePath, openBunDatabase)

describe("openagents_desktop.sync.host_owned_sqlite.v1", () => {
  test("closes the database when initialization fails", () => {
    let closed = false
    expect(() =>
      openDesktopSyncStore("fixture.sqlite", () => ({
        exec: () => {
          throw new Error("migration failed")
        },
        prepare: () => {
          throw new Error("unexpected prepare")
        },
        close: () => {
          closed = true
        },
      })),
    ).toThrow()
    expect(closed).toBe(true)
  })

  test("persists one installation identity and reuses it after restart", () => {
    const root = mkdtempSync(path.join(tmpdir(), "openagents-desktop-sync-"))
    const databasePath = path.join(root, "private", "sync.sqlite")
    let generated = 0
    const randomId = () => `fixture-${++generated}`
    try {
      const first = openDesktopSyncHost({ databasePath, randomId, openStore: openTestStore })
      expect(first.status()).toEqual({
        state: "local_ready",
        schemaVersion: 1,
        identityState: "persisted",
        pendingMutationCount: 0,
      })
      first.close()

      const inspectFirst = openTestStore(databasePath)
      const identity = Effect.runSync(inspectFirst.identity())
      Effect.runSync(inspectFirst.close())
      expect(identity).toMatchObject({
        clientGroupId: "openagents-desktop.fixture-1",
        clientId: "desktop.fixture-2",
        schemaVersion: 1,
      })

      const second = openDesktopSyncHost({ databasePath, randomId, openStore: openTestStore })
      expect(generated).toBe(2)
      second.close()
      second.close()
      expect(second.status().state).toBe("closed")

      const inspectSecond = openTestStore(databasePath)
      expect(Effect.runSync(inspectSecond.identity())).toEqual(identity)
      Effect.runSync(inspectSecond.close())
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("creates an owner-private directory and database on POSIX hosts", () => {
    const root = mkdtempSync(path.join(tmpdir(), "openagents-desktop-sync-mode-"))
    const databasePath = path.join(root, "private", "sync.sqlite")
    try {
      const host = openDesktopSyncHost({ databasePath, randomId: () => "mode", openStore: openTestStore })
      if (process.platform !== "win32") {
        expect(statSync(path.dirname(databasePath)).mode & 0o777).toBe(0o700)
        expect(statSync(databasePath).mode & 0o777).toBe(0o600)
      }
      host.close()
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("projects no path, client identity, token, or database handle", () => {
    const root = mkdtempSync(path.join(tmpdir(), "openagents-desktop-sync-projection-"))
    try {
      const host = openDesktopSyncHost({
        databasePath: path.join(root, "private", "sync.sqlite"),
        randomId: () => "private-ref",
        openStore: openTestStore,
      })
      const serialized = JSON.stringify(host.status())
      expect(serialized).not.toContain(root)
      expect(serialized).not.toContain("private-ref")
      expect(serialized).not.toContain("token")
      expect(serialized).not.toContain("sqlite")
      host.close()
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})
