import assert from "node:assert/strict"
import { describe, it } from "node:test"
import { sqliteDatabaseConformanceCases } from "./conformance.ts"
import { openSqliteDatabase } from "./open.ts"
import { detectSqliteRuntime } from "./sqlite-database.ts"

/**
 * Node half of the dual-runtime proof (BUN-1, openagents#8779). This file is
 * deliberately named WITHOUT `.test.` so `bun test` never picks it up: it
 * runs under REAL Node (`node --test`, native type stripping — Node >= 23.6)
 * via the package's `test:node` script, which is the honest way to prove the
 * `node:sqlite` path — no mocked `process.versions.bun`, no Bun shim of
 * `node:sqlite`.
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
