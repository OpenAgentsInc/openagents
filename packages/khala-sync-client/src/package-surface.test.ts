import { describe, expect, test } from "bun:test"
import * as rootSurface from "./index.js"
import * as webSurface from "./web/index.js"

const packageRoot = new URL("../", import.meta.url)
const read = (path: string): Promise<string> =>
  Bun.file(new URL(path, packageRoot)).text()

describe("@openagentsinc/khala-sync-client package surface", () => {
  test("keeps the root entry free of the Bun-only SQLite store", async () => {
    const [packageJsonText, rootEntry] = await Promise.all([
      read("package.json"),
      read("src/index.ts"),
    ])
    const packageJson = JSON.parse(packageJsonText) as {
      exports?: Record<string, string>
    }

    expect(packageJson.exports?.["."]).toBe("./src/index.ts")
    expect(packageJson.exports?.["./sqlite-store"]).toBe("./src/sqlite-store.ts")
    expect(rootEntry).not.toContain('from "./sqlite-store.js"')
    expect(rootEntry).not.toContain("openKhalaSyncStore")
    expect(rootSurface.createHttpKhalaSyncTransport).toBeFunction()
    expect(webSurface.openKhalaSyncWasmStore).toBeFunction()
  })
})
