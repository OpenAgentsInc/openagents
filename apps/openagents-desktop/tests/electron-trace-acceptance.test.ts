import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import path from "node:path"
import { traceAcceptanceJourney, traceAcceptanceReload } from "../src/electron-trace-acceptance.ts"

const appRoot = path.resolve(import.meta.dir, "..")

describe("openagents_desktop.seam.codex_trace_electron_acceptance.v1", () => {
  test("the built-Electron journey names every video-blocking regression", () => {
    for (const marker of [
      "blank_shell", "stale_loading_copy", "known_title_fallback",
      "child_leaked_to_sidebar", "descendants_hidden", "silent_loss",
      "keyboard_tree_stuck", "tool_row_inaccessible", "inspector_inaccessible",
      "timeline_not_scrollable", "timeline_scroll_stuck", "ref_restore_missing",
    ]) expect(traceAcceptanceJourney).toContain(marker)
    expect(traceAcceptanceReload).toContain("restart_expectation_missing")
    expect(traceAcceptanceReload).toContain("itemInspectorRestored")
  })

  test("the receipt projection is public-safe", () => {
    const returnExpression = traceAcceptanceJourney.slice(traceAcceptanceJourney.lastIndexOf("return {ok:true"))
    for (const forbidden of ["title", "threadRef", "itemRef", "summary", "path", "text", "credential", "token"]) {
      expect(returnExpression.toLowerCase()).not.toContain(forbidden.toLowerCase())
    }
    for (const timing of ["shellReadyMs", "catalogReadyMs", "pageReadyMs", "inspectorReadyMs"]) {
      expect(returnExpression).toContain(timing)
    }
  })

  test("the normal smoke runs the journey and a real renderer reload", () => {
    const main = readFileSync(path.join(appRoot, "src/main.ts"), "utf8")
    expect(main).toContain('step("codex-trace-acceptance", traceAcceptanceJourney)')
    expect(main).toContain('step("codex-trace-reload-restoration", traceAcceptanceReload)')
    expect(main).toContain("window.webContents.reload()")
  })
})
