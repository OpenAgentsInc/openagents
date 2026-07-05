import { Database, type SQLQueryBindings } from "bun:sqlite"
import { describe, expect, test } from "bun:test"

import {
  KHALA_MOBILE_PREFERENCES_DB_NAME,
  KHALA_MOBILE_PREFERENCES_KIND,
  KhalaNonsecretPreferenceError,
  khalaNonsecretPreferenceKeys,
  openKhalaNonsecretPreferences,
  type ExpoPreferenceSqliteDatabase,
  type ExpoPreferenceSqliteModule,
} from "../src/preferences/nonsecret-preferences"

const expoSqliteFromBun = (): {
  module: ExpoPreferenceSqliteModule
  statements: Array<string>
} => {
  const databases = new Map<string, Database>()
  const statements: Array<string> = []

  const open = (name: string): ExpoPreferenceSqliteDatabase => {
    const db = databases.get(name) ?? new Database(":memory:")
    databases.set(name, db)
    return {
      execAsync: async statement => {
        statements.push(statement)
        db.exec(statement)
      },
      getFirstAsync: async <T>(statement: string, ...params: ReadonlyArray<unknown>) =>
        (db.query(statement).get(...(params as ReadonlyArray<SQLQueryBindings>)) as T | null) ?? null,
      runAsync: async (statement, ...params) => {
        statements.push(statement)
        db.query(statement).run(...(params as ReadonlyArray<SQLQueryBindings>))
      },
    }
  }

  return {
    module: {
      openDatabaseAsync: async name => open(name),
    },
    statements,
  }
}

describe("Khala nonsecret preferences", () => {
  test("stores typed nonsecret preferences in a dedicated SQLite database", async () => {
    const sqlite = expoSqliteFromBun()
    const store = await openKhalaNonsecretPreferences({
      sqliteLoader: async () => sqlite.module,
    })

    expect(store.kind).toBe(KHALA_MOBILE_PREFERENCES_KIND)
    expect(store.databaseName).toBe(KHALA_MOBILE_PREFERENCES_DB_NAME)
    expect(sqlite.statements[0]).toContain("khala_nonsecret_preferences")

    expect(await store.get("threadListDisplayMode")).toBe("comfortable")
    await store.set("threadListDisplayMode", "compact")
    expect(await store.get("threadListDisplayMode")).toBe("compact")
    await store.reset("threadListDisplayMode")
    expect(await store.get("threadListDisplayMode")).toBe("comfortable")
  })

  test("round-trips safe boolean onboarding hints", async () => {
    const sqlite = expoSqliteFromBun()
    const store = await openKhalaNonsecretPreferences({
      sqliteLoader: async () => sqlite.module,
    })

    expect(await store.get("hasSeenTailnetPairingHint")).toBe(false)
    await store.set("hasSeenTailnetPairingHint", true)
    expect(await store.get("hasSeenTailnetPairingHint")).toBe(true)
  })

  test("keeps the allowed key surface small and nonsecret", () => {
    expect(khalaNonsecretPreferenceKeys).toEqual([
      "hasSeenTailnetPairingHint",
      "threadListDisplayMode",
    ])
    expect(khalaNonsecretPreferenceKeys.join(" ")).not.toMatch(
      /token|secret|apiKey|messageBody|prompt/i,
    )
  })

  test("rejects secret-shaped or unknown keys and invalid values", async () => {
    const sqlite = expoSqliteFromBun()
    const store = await openKhalaNonsecretPreferences({
      sqliteLoader: async () => sqlite.module,
    })

    await expect(store.get("authToken" as never)).rejects.toThrow(KhalaNonsecretPreferenceError)
    await expect(store.set("chatBody" as never, "hello" as never)).rejects.toThrow(
      KhalaNonsecretPreferenceError,
    )
    await expect(store.set("threadListDisplayMode", "dense" as never)).rejects.toThrow(
      KhalaNonsecretPreferenceError,
    )
  })
})
