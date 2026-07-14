import { describe, expect, test } from "vite-plus/test"
import { readFileSync } from "node:fs"
import path from "node:path"
import { traceAcceptanceJourney, traceAcceptanceReload } from "../src/electron-trace-acceptance.ts"

const appRoot = path.resolve(import.meta.dirname, "..")

describe("openagents_desktop.seam.codex_trace_electron_acceptance.v1", () => {
  test("the built-Electron journey names every video-blocking regression", () => {
    for (const marker of [
      "blank_shell", "stale_loading_copy", "known_title_fallback",
      "child_leaked_to_sidebar", "descendants_hidden", "silent_loss",
      "keyboard_tree_stuck", "tool_row_inaccessible", "inspector_inaccessible",
      "agent_handoff_missing", "protocol_metadata_visible", "agent_handoff_card_incomplete", "agent_handoff_inspector_incomplete",
      "timeline_not_scrollable", "timeline_scroll_stuck", "ref_restore_missing",
      "inline_agent_preview_missing", "inline_agent_preview_stale", "inline_agent_navigation_failed", "inline_agent_return_failed",
      "agent_metadata_missing", "agent_metadata_not_collapsed", "agent_metadata_expand_failed", "agent_metadata_replaced_tree", "agent_metadata_collapse_failed",
      "agent_status_word_visible", "agent_status_icon_missing",
      "history_shortcut_down_failed", "history_shortcut_up_failed",
      "history_shortcut_hold_failed", "history_shortcut_offscreen", "history_shortcut_hold_return_failed",
      "history_modifier_hint_missing", "history_modifier_scroll_reset",
      // EP250 bottom-anchored autoload regressions.
      "pager_still_present", "not_bottom_anchored", "scrollup_prefetch_failed",
      "position_caption_stale", "prepend_anchor_lost",
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

  test("handoff acceptance keys on typed kind and fields rather than display copy", () => {
    expect(traceAcceptanceJourney).toContain("kind===handoffItem.kind")
    expect(traceAcceptanceJourney).not.toContain("textContent==='Agent message'")
  })

  test("does not refetch the already-selected agent before clicking its tool row", () => {
    expect(traceAcceptanceJourney).toContain("agentButton?.getAttribute('aria-selected') !== 'true'")
  })

  test("modifier hints target a rendered Codex root, never an arbitrary merged-provider root", () => {
    expect(traceAcceptanceJourney).toContain("visibleRoots[0].threadRef")
    expect(traceAcceptanceJourney).not.toContain("roots[0].threadRef")
  })
})
