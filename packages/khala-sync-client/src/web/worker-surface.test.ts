import { describe, expect, test } from "bun:test"
import {
  KHALA_SYNC_SAH_POOL_DIRECTORY,
  KHALA_SYNC_WEB_DB_FILENAME,
  startKhalaSyncStorageWorker,
} from "./sqlite-wasm-worker.js"

describe("@openagentsinc/khala-sync-client web worker surface", () => {
  test("exports the worker bootstrap and stable storage defaults", () => {
    expect(startKhalaSyncStorageWorker).toBeFunction()
    expect(KHALA_SYNC_WEB_DB_FILENAME).toBe("/khala-sync.sqlite3")
    expect(KHALA_SYNC_SAH_POOL_DIRECTORY).toBe(".khala-sync-sahpool")
  })
})
