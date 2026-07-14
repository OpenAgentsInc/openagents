import assert from "node:assert/strict"
import { describe, it } from "vite-plus/test"
import { sqliteDatabaseConformanceCases } from "./conformance.ts"
import { openSqliteDatabase } from "./open.ts"
import { detectSqliteRuntime } from "./sqlite-database.ts"

/**
 * Node half of the dual-runtime proof (BUN-1, openagents#8779). This file is
 * This suite runs in Vite Plus's Node project against the real `node:sqlite`
 * implementation. It does not mock runtime detection or shim `node:sqlite`.
 */

describe("sqlite-runtime under Node", () => {
  it("detects the node runtime", () => {
    assert.equal(detectSqliteRuntime(), "node")
  })

  for (const conformanceCase of sqliteDatabaseConformanceCases(
    (path) => openSqliteDatabase(path),
    "node",
  )) {
    it(conformanceCase.name, () => {
      conformanceCase.run()
    })
  }
})
