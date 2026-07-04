import { Database } from "bun:sqlite"
import { describe, expect, test } from "bun:test"
import {
  createKhalaSyncStoreCore,
  localStoreFromCore,
  type SqlDriver,
  type SqlValue,
} from "./store-core.js"
import { KhalaSyncClientStoreError } from "./store.js"
import { describeKhalaSyncStoreSemantics } from "./store-semantics.testkit.js"

/**
 * KS-5.4: the driver-agnostic SQL core carries ALL store semantics; both
 * the desktop `bun:sqlite` store and the web SQLite-WASM storage worker
 * are thin drivers around it. Here the full shared semantics suite runs
 * against the core directly, with `bun:sqlite` as the harness driver.
 */

const bunDriver = (db: Database): SqlDriver => ({
  exec: (sql) => db.exec(sql),
  run: (sql, params = []) => {
    db.query(sql).run(...(params as Array<SqlValue>))
  },
  all: <Row>(sql: string, params: ReadonlyArray<SqlValue> = []) =>
    db.query(sql).all(...(params as Array<SqlValue>)) as ReadonlyArray<Row>,
  transaction: (fn) => db.transaction(fn)(),
})

describeKhalaSyncStoreSemantics(
  "store core (bun:sqlite harness driver)",
  () => {
    const db = new Database(":memory:")
    return {
      store: localStoreFromCore(createKhalaSyncStoreCore(bunDriver(db))),
      cleanup: () => db.close(),
    }
  },
)

describe("createKhalaSyncStoreCore", () => {
  test("wraps schema-migration failure in the typed error taxonomy", () => {
    const broken: SqlDriver = {
      exec: () => {
        throw new Error("disk I/O error")
      },
      run: () => {},
      all: () => [],
      transaction: (fn) => fn(),
    }
    expect(() => createKhalaSyncStoreCore(broken)).toThrow(
      KhalaSyncClientStoreError,
    )
    try {
      createKhalaSyncStoreCore(broken)
    } catch (error) {
      expect((error as KhalaSyncClientStoreError).reason).toBe(
        "storage_failure",
      )
    }
  })
})
