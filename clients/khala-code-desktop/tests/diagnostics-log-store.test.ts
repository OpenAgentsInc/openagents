import { describe, expect, test } from "bun:test"

import { KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER } from "../src/shared/diagnostics-redaction"
import { createKhalaCodeDesktopDiagnosticsLogStore } from "../src/shared/diagnostics-log-store"

describe("createKhalaCodeDesktopDiagnosticsLogStore", () => {
  test("records entries into the correct category bucket", () => {
    const store = createKhalaCodeDesktopDiagnosticsLogStore()
    store.record({ category: "main", message: "boot" })
    store.record({ category: "renderer", message: "mounted" })
    store.record({ category: "service", message: "codex-app-server ready", source: "codex-app-server" })
    store.record({ category: "native-shell", message: "window created" })

    const snapshot = store.snapshot()
    expect(snapshot.entriesByCategory.main).toHaveLength(1)
    expect(snapshot.entriesByCategory.renderer).toHaveLength(1)
    expect(snapshot.entriesByCategory.service).toHaveLength(1)
    expect(snapshot.entriesByCategory.service[0]?.source).toBe("codex-app-server")
    expect(snapshot.entriesByCategory["native-shell"]).toHaveLength(1)
  })

  test("redacts message and context text before storing, never retaining raw secrets", () => {
    const store = createKhalaCodeDesktopDiagnosticsLogStore()
    store.record({
      category: "main",
      context: { apiKey: "sk-abcdefghijklmnopqrstuvwx", threadId: "thread-1" },
      message: "provider call failed with key sk-abcdefghijklmnopqrstuvwx",
    })
    const [entry] = store.snapshot().entriesByCategory.main
    expect(entry?.message).not.toContain("sk-abcdefghijklmnopqrstuvwx")
    expect(entry?.context?.apiKey).toBe(KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER)
    expect(entry?.context?.threadId).toBe("thread-1")
  })

  test("tracks fatal entries separately in addition to their category bucket", () => {
    const store = createKhalaCodeDesktopDiagnosticsLogStore()
    store.record({ category: "main", fatal: true, level: "error", message: "uncaught exception" })
    store.record({ category: "main", message: "ordinary log line" })

    const snapshot = store.snapshot()
    expect(snapshot.entriesByCategory.main).toHaveLength(2)
    expect(snapshot.fatalEntries).toHaveLength(1)
    expect(snapshot.fatalEntries[0]?.message).toBe("uncaught exception")
  })

  test("bounds each category to the configured limit, dropping oldest first", () => {
    const store = createKhalaCodeDesktopDiagnosticsLogStore({ limitPerCategory: 3 })
    for (let i = 0; i < 5; i += 1) {
      store.record({ category: "main", message: `entry-${i}` })
    }
    const entries = store.snapshot().entriesByCategory.main
    expect(entries).toHaveLength(3)
    expect(entries.map(entry => entry.message)).toEqual(["entry-2", "entry-3", "entry-4"])
  })

  test("bounds the fatal ring buffer independently of category limits", () => {
    const store = createKhalaCodeDesktopDiagnosticsLogStore({ fatalLimit: 2, limitPerCategory: 50 })
    for (let i = 0; i < 4; i += 1) {
      store.record({ category: "main", fatal: true, message: `fatal-${i}` })
    }
    const snapshot = store.snapshot()
    expect(snapshot.fatalEntries).toHaveLength(2)
    expect(snapshot.fatalEntries.map(entry => entry.message)).toEqual(["fatal-2", "fatal-3"])
  })

  test("clear() empties every category and the fatal buffer", () => {
    const store = createKhalaCodeDesktopDiagnosticsLogStore()
    store.record({ category: "main", fatal: true, message: "boom" })
    store.record({ category: "renderer", message: "hi" })
    store.clear()
    const snapshot = store.snapshot()
    for (const bucket of Object.values(snapshot.entriesByCategory)) expect(bucket).toHaveLength(0)
    expect(snapshot.fatalEntries).toHaveLength(0)
  })

  test("defaults level to info and fatal to false", () => {
    const store = createKhalaCodeDesktopDiagnosticsLogStore()
    const entry = store.record({ category: "main", message: "plain" })
    expect(entry.level).toBe("info")
    expect(entry.fatal).toBe(false)
  })
})
