import { describe, expect, test } from "vite-plus/test"
import { Schema } from "effect"

import {
  IdeNavigationEntryRefSchema,
  IdeWorkbenchStateSchema,
  breadcrumbsForPath,
  emptyIdeNavigationHistory,
  emptyIdeWorkbenchState,
  exportIdeEditorSettings,
  importIdeEditorSettings,
  markIdeNavigationUnavailable,
  pushIdeNavigation,
  rankIdeQuickOpen,
  resolveIdeEditorSetting,
  setIdeEditorSetting,
  stepIdeNavigation,
} from "./workbench-contract.ts"
import { IdeProjectRefSchema, IdeRootRefSchema, IdeWorktreeRefSchema } from "./project-contract.ts"
import { IdeDocumentGeneration, makeIdeDocumentRef } from "./monaco-document-contract.ts"

const navigationEntry = (ordinal: number, pathRef: string) => ({
  entryRef: IdeNavigationEntryRefSchema.make(`ide.navigation.test-${ordinal}`),
  source: "quick_open" as const,
  projectRef: IdeProjectRefSchema.make("ide.project.test"),
  rootRef: IdeRootRefSchema.make("ide.root.test"),
  worktreeRef: IdeWorktreeRefSchema.make("ide.worktree.test"),
  documentRef: makeIdeDocumentRef("grant.test", ordinal),
  generation: IdeDocumentGeneration.make(ordinal),
  pathRef,
  selection: { start: ordinal, end: ordinal },
  state: "ready" as const,
  reason: null,
})

describe("IDE-04 schema-first workbench contract", () => {
  test("navigation preserves exact document/worktree identity and unavailable history", () => {
    let history = emptyIdeNavigationHistory()
    history = pushIdeNavigation(history, navigationEntry(1, "src/a.ts"))
    history = pushIdeNavigation(history, navigationEntry(2, "src/a.ts"))
    expect(history.entries).toHaveLength(2)
    const stepped = stepIdeNavigation(history, "back")
    expect(stepped.entry?.documentRef).toBe(history.entries[0]?.documentRef)
    const unavailable = markIdeNavigationUnavailable(stepped.history, stepped.entry!.entryRef, "generation replaced")
    expect(unavailable.entries[0]).toMatchObject({ state: "unavailable", reason: "generation replaced" })
  })

  test("quick open is bounded, deterministic, fuzzy, and path-only", () => {
    const ranked = rankIdeQuickOpen("we", ["src/workbench.ts", "src/web.ts", "README.md"], 2)
    expect(ranked.phase).toBe("ready")
    expect(ranked.results).toHaveLength(2)
    expect(ranked.results.map(result => result.pathRef)).toContain("src/workbench.ts")
    expect(rankIdeQuickOpen("secret", ["src/a.ts"]).phase).toBe("empty")
  })

  test("breadcrumbs and pre-language Outline remain honest", () => {
    const state = emptyIdeWorkbenchState()
    expect(state.outline._tag).toBe("Unavailable")
    expect(breadcrumbsForPath("src/ide/workbench.ts").map(item => item.label)).toEqual([
      "Project", "src", "ide", "workbench.ts",
    ])
    expect(Schema.decodeUnknownSync(IdeWorkbenchStateSchema)(state)).toEqual(state)
  })

  test("settings resolve workspace over user over default and import exactly", () => {
    let settings = emptyIdeWorkbenchState().settings
    settings = setIdeEditorSetting(settings, { id: "editor.tabSize", scope: "user", value: { _tag: "Integer", value: 4 } })
    settings = setIdeEditorSetting(settings, { id: "editor.tabSize", scope: "workspace", value: { _tag: "Integer", value: 8 } })
    expect(resolveIdeEditorSetting(settings, "editor.tabSize")).toEqual({ value: { _tag: "Integer", value: 8 }, source: "workspace" })
    expect(importIdeEditorSettings(exportIdeEditorSettings(settings))).toEqual(settings)
    expect(importIdeEditorSettings('{"overrides":[{"id":"unknown"}]}').errors).toHaveLength(1)
  })
})
