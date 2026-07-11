import { describe, expect, test } from "bun:test"
import * as rootEntryModule from "./index.js"
import * as webEntryModule from "./web/index.js"

const packageRoot = new URL("../", import.meta.url)
const read = (path: string): Promise<string> =>
  Bun.file(new URL(path, packageRoot)).text()

describe("@openagentsinc/khala-sync-client package surface", () => {
  test("keeps the root entry free of runtime-specific SQLite stores", async () => {
    const [packageJsonText, rootEntry] = await Promise.all([
      read("package.json"),
      read("src/index.ts"),
    ])
    const packageJson = JSON.parse(packageJsonText) as {
      exports?: Record<string, string>
    }

    expect(packageJson.exports?.["."]).toBe("./src/index.ts")
    expect(packageJson.exports?.["./sqlite-store"]).toBe("./src/sqlite-store.ts")
    expect(packageJson.exports?.["./expo-sqlite-store"]).toBe(
      "./src/expo-sqlite-store.ts",
    )
    expect(rootEntry).not.toContain('from "./sqlite-store.js"')
    expect(rootEntry).not.toContain('from "./expo-sqlite-store.js"')
    expect(rootEntry).not.toContain("openKhalaSyncStore")
  })

  test("the root barrel loads and re-exports the engine surface", () => {
    // A real import (not a text read): a broken re-export in index.ts
    // fails this test at module-load time.
    expect(typeof rootEntryModule.createHttpKhalaSyncTransport).toBe("function")
    expect(typeof rootEntryModule.createKhalaSyncSession).toBe("function")
    expect(typeof rootEntryModule.createOverlay).toBe("function")
    expect(typeof rootEntryModule.createChatClientMutators).toBe("function")
    expect(typeof rootEntryModule.createKhalaSyncConversation).toBe("function")
    expect(typeof rootEntryModule.createKhalaSyncAgentTimeline).toBe("function")
  })

  test("the web barrel loads without pulling the WASM bundle", () => {
    expect(typeof webEntryModule.electWriter).toBe("function")
    expect(typeof webEntryModule.isStoreRequest).toBe("function")
  })
})
