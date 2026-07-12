import { describe, expect, test } from "bun:test"
import { IntentRef, StaticPayload, resolveIntentRef, type View } from "@effect-native/core"
import { Effect, SubscriptionRef } from "@effect-native/core/effect"

import type { DesktopWorkspaceDocument } from "../workspace-contract.ts"
import {
  emptyWorkspaceEditorState,
  makeWorkspaceEditorHandlers,
  withWorkspaceEditorEvent,
  withWorkspaceEditorExternalResult,
  withWorkspaceEditorFind,
  withWorkspaceEditorFindStep,
  withWorkspaceEditorOpened,
  withWorkspaceEditorOpening,
  withWorkspaceEditorRedo,
  withWorkspaceEditorSaveResult,
  withWorkspaceEditorUndo,
  workspaceEditorIntents,
  workspaceEditorRecoverySnapshot,
  workspaceEditorTabDirty,
  workspaceEditorView,
  type WorkspaceDocumentBridge,
  type WorkspaceEditorState,
} from "./workspace-editor.ts"

const { makeIntentRegistry } = await import("@effect-native/core")

type AnyNode = Readonly<Record<string, unknown>>
const collectNodes = (root: unknown): Array<AnyNode> => {
  const found: AnyNode[] = []
  const walk = (value: unknown): void => {
    if (Array.isArray(value)) { for (const item of value) walk(item); return }
    if (typeof value !== "object" || value === null) return
    const node = value as AnyNode
    if (typeof node._tag === "string") found.push(node)
    for (const [key, child] of Object.entries(node)) {
      if (key === "_tag" || key === "style" || key === "a11y") continue
      walk(child)
    }
  }
  walk(root)
  return found
}
const nodeByKey = (view: View, key: string): AnyNode | undefined =>
  collectNodes(view).find(node => node.key === key)

const document = (overrides: Partial<DesktopWorkspaceDocument> = {}): DesktopWorkspaceDocument => ({
  grantRef: "workspace.grant.editor",
  pathRef: "src/index.ts",
  content: "const needle = true\nconst other = needle\n",
  revisionRef: "workspace.document.initial",
  languageMode: "typescript",
  encoding: "utf-8",
  lineEnding: "lf",
  sizeBytes: 46,
  ...overrides,
})

const readyState = (doc: DesktopWorkspaceDocument = document()): WorkspaceEditorState =>
  withWorkspaceEditorOpened(
    withWorkspaceEditorOpening(emptyWorkspaceEditorState(), doc.pathRef),
    doc.pathRef,
    { state: "available", document: doc },
  )

describe("workspace editor state", () => {
  test("opens and selects bounded relative tabs", () => {
    let state = readyState()
    state = withWorkspaceEditorOpening(state, "README.md")
    state = withWorkspaceEditorOpened(state, "README.md", {
      state: "available",
      document: document({ pathRef: "README.md", content: "hello", languageMode: "markdown" }),
    })
    expect(state.tabs.map(tab => tab.pathRef)).toEqual(["src/index.ts", "README.md"])
    expect(state.activePathRef).toBe("README.md")
    expect(state.tabs[1]?.phase).toBe("ready")
  })

  test("typed changes own dirty state, bounded undo/redo, and selection", () => {
    const initial = readyState()
    const changed = withWorkspaceEditorEvent(initial, { type: "change", value: "first edit" })
    expect(workspaceEditorTabDirty(changed.tabs[0]!)).toBe(true)
    expect(changed.tabs[0]?.undo).toEqual([document().content])
    const selected = withWorkspaceEditorEvent(changed, { type: "selection", start: 2, end: 8 })
    expect(selected.tabs[0]?.selection).toEqual({ start: 2, end: 8 })
    const undone = withWorkspaceEditorUndo(selected)
    expect(undone.tabs[0]?.draft).toBe(document().content)
    expect(workspaceEditorTabDirty(undone.tabs[0]!)).toBe(false)
    const redone = withWorkspaceEditorRedo(undone)
    expect(redone.tabs[0]?.draft).toBe("first edit")

    let bounded = initial
    for (let index = 0; index < 120; index += 1) {
      bounded = withWorkspaceEditorEvent(bounded, { type: "change", value: `edit ${index}` })
    }
    expect(bounded.tabs[0]?.undo).toHaveLength(100)
  })

  test("find is bounded, wraps, and projects the active selection", () => {
    let state = withWorkspaceEditorFind(readyState(), "needle")
    expect(state.tabs[0]?.findMatches).toEqual([6, 34])
    state = withWorkspaceEditorFindStep(state, 1)
    expect(state.tabs[0]?.findIndex).toBe(1)
    expect(state.tabs[0]?.selection).toEqual({ start: 34, end: 40 })
    expect(state.tabs[0]?.selectionVersion).toBe(1)
    state = withWorkspaceEditorFindStep(state, 1)
    expect(state.tabs[0]?.findIndex).toBe(0)
  })

  test("save success resets the base while conflict preserves the local draft", () => {
    const changed = withWorkspaceEditorEvent(readyState(), { type: "change", value: "local draft" })
    const external = document({ content: "external", revisionRef: "workspace.document.external" })
    const conflict = withWorkspaceEditorSaveResult(changed, "src/index.ts", { state: "conflict", current: external })
    expect(conflict.tabs[0]).toMatchObject({ phase: "conflict", draft: "local draft", externalDocument: external })
    const savedDocument = document({ content: "local draft", revisionRef: "workspace.document.saved" })
    const saved = withWorkspaceEditorSaveResult(conflict, "src/index.ts", { state: "saved", document: savedDocument })
    expect(saved.tabs[0]).toMatchObject({ phase: "ready", draft: "local draft", document: savedDocument, saveState: "saved" })
    expect(workspaceEditorTabDirty(saved.tabs[0]!)).toBe(false)
  })

  test("external changes reload clean tabs and conflict without losing dirty drafts", () => {
    const external = document({ content: "external", revisionRef: "workspace.document.external" })
    const clean = withWorkspaceEditorExternalResult(readyState(), "src/index.ts", { state: "available", document: external })
    expect(clean.tabs[0]).toMatchObject({ phase: "ready", draft: "external", document: external })

    const dirty = withWorkspaceEditorEvent(readyState(), { type: "change", value: "local draft" })
    const conflict = withWorkspaceEditorExternalResult(dirty, "src/index.ts", { state: "available", document: external })
    expect(conflict.tabs[0]).toMatchObject({
      phase: "conflict",
      draft: "local draft",
      externalDocument: external,
    })
  })

  test("external deletion preserves a dirty draft for recovery", () => {
    const dirty = withWorkspaceEditorEvent(readyState(), { type: "change", value: "local draft" })
    const missing = withWorkspaceEditorExternalResult(dirty, "src/index.ts", {
      state: "unavailable",
      reason: "missing",
      message: "The document no longer exists.",
    })
    expect(missing.tabs[0]).toMatchObject({ phase: "conflict", draft: "local draft", externalDocument: null })
    expect(missing.tabs[0]?.reason).toContain("preserved")
  })

  test("recovery snapshot contains bounded relative refs and drafts, never a root", () => {
    const state = withWorkspaceEditorEvent(readyState(), { type: "change", value: "recover me" })
    const snapshot = workspaceEditorRecoverySnapshot(state)
    expect(snapshot).toEqual({
      version: 1,
      activePathRef: "src/index.ts",
      tabs: [{
        pathRef: "src/index.ts",
        grantRef: "workspace.grant.editor",
        expectedRevisionRef: "workspace.document.initial",
        draft: "recover me",
      }],
    })
    expect(JSON.stringify(snapshot)).not.toContain("/Users/")
  })
})

describe("workspace editor Effect Native view", () => {
  test("empty state explains how to open a document", () => {
    const view = workspaceEditorView(emptyWorkspaceEditorState())
    expect(nodeByKey(view, "workspace-editor-empty-title")?.content).toBe("No document open")
  })

  test("ready state lowers to the replaceable code-editor Host contract", () => {
    const view = workspaceEditorView(readyState())
    const host = nodeByKey(view, "workspace-editor-host-src/index.ts")
    expect(host?._tag).toBe("Host")
    expect(host?.kind).toBe("code-editor")
    expect(host?.props).toMatchObject({
      value: document().content,
      language: "typescript",
      readOnly: false,
      wordWrap: false,
      minimap: false,
      selection: { start: 0, end: 0, version: 0 },
    })
    expect((host?.onEvent as { name?: string }).name).toBe("WorkspaceEditorEventReceived")
    expect(JSON.stringify(view)).not.toContain("workspace.grant.editor")
    expect(JSON.stringify(view)).not.toContain("workspace.document.initial")
  })

  test("find and undo project versioned authoritative selections to the host", () => {
    let state = withWorkspaceEditorFind(readyState(), "needle")
    state = withWorkspaceEditorFindStep(state, 1)
    let host = nodeByKey(workspaceEditorView(state), "workspace-editor-host-src/index.ts")
    expect(host?.props).toMatchObject({ selection: { start: 34, end: 40, version: 1 } })

    state = withWorkspaceEditorEvent(state, { type: "change", value: "edited" })
    state = withWorkspaceEditorUndo(state)
    host = nodeByKey(workspaceEditorView(state), "workspace-editor-host-src/index.ts")
    expect(host?.props).toMatchObject({
      value: document().content,
      selection: { start: document().content.length, end: document().content.length, version: 2 },
    })
  })

  test("tabs, find, save, and dirty close confirmation use familiar inline controls", () => {
    const dirty = withWorkspaceEditorEvent(readyState(), { type: "change", value: "dirty" })
    const view = workspaceEditorView({ ...dirty, closeConfirmRef: "src/index.ts" })
    expect(nodeByKey(view, "workspace-editor-tab-src/index.ts")?.label).toContain("•")
    expect(nodeByKey(view, "workspace-editor-find-query")?._tag).toBe("TextField")
    expect(nodeByKey(view, "workspace-editor-save")?.disabled).toBe(false)
    expect(nodeByKey(view, "workspace-editor-close")?.label).toBe("Discard changes")
    expect(nodeByKey(view, "workspace-editor-close-cancel")?.label).toBe("Keep editing")
  })

  test("conflict state offers explicit reload or overwrite choices", () => {
    const dirty = withWorkspaceEditorEvent(readyState(), { type: "change", value: "mine" })
    const conflict = withWorkspaceEditorSaveResult(dirty, "src/index.ts", {
      state: "conflict",
      current: document({ content: "theirs", revisionRef: "workspace.document.theirs" }),
    })
    const view = workspaceEditorView(conflict)
    expect(nodeByKey(view, "workspace-editor-conflict-copy")?.content).toContain("Changed outside")
    expect(nodeByKey(view, "workspace-editor-conflict-reload")?.label).toBe("Reload theirs")
    expect(nodeByKey(view, "workspace-editor-conflict-keep")?.label).toBe("Save mine")
  })
})

const makeBridge = (): {
  bridge: WorkspaceDocumentBridge
  calls: Array<{ op: string; value: unknown }>
} => {
  const calls: Array<{ op: string; value: unknown }> = []
  return {
    calls,
    bridge: {
      openWorkspaceDocument: async value => {
        calls.push({ op: "open", value })
        return { state: "available", document: document() }
      },
      saveWorkspaceDocument: async value => {
        calls.push({ op: "save", value })
        const request = value as { content: string }
        return { state: "saved", document: document({ content: request.content, revisionRef: "workspace.document.saved" }) }
      },
    },
  }
}

const pressIntent = (view: View, key: string) => {
  const node = nodeByKey(view, key) as { onPress: Parameters<typeof resolveIntentRef>[0] }
  return resolveIntentRef(node.onPress, null)
}

describe("workspace editor typed intent loop", () => {
  test("open, edit, and Save dispatch exact grant/ref/revision requests", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const { bridge, calls } = makeBridge()
      const state = yield* SubscriptionRef.make({ workspaceEditor: emptyWorkspaceEditorState() })
      const registry = yield* makeIntentRegistry(workspaceEditorIntents, makeWorkspaceEditorHandlers(state, bridge))
      yield* registry.dispatch(resolveIntentRef(IntentRef("WorkspaceEditorOpenRequested", StaticPayload({ grantRef: "workspace.grant.editor", pathRef: "src/index.ts" })), null))
      yield* registry.dispatch(resolveIntentRef(IntentRef("WorkspaceEditorEventReceived", StaticPayload({ type: "change", value: "edited" })), null))
      const view = workspaceEditorView((yield* SubscriptionRef.get(state)).workspaceEditor)
      yield* registry.dispatch(pressIntent(view, "workspace-editor-save"))
      expect(calls).toEqual([
        { op: "open", value: { grantRef: "workspace.grant.editor", pathRef: "src/index.ts" } },
        { op: "save", value: {
          grantRef: "workspace.grant.editor",
          pathRef: "src/index.ts",
          content: "edited",
          expectedRevisionRef: "workspace.document.initial",
        } },
      ])
      const tab = (yield* SubscriptionRef.get(state)).workspaceEditor.tabs[0]!
      expect(tab.saveState).toBe("saved")
      expect(workspaceEditorTabDirty(tab)).toBe(false)
    }))
  })

  test("dirty close requires a second explicit confirmation", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const { bridge } = makeBridge()
      const state = yield* SubscriptionRef.make({ workspaceEditor: withWorkspaceEditorEvent(readyState(), { type: "change", value: "dirty" }) })
      const registry = yield* makeIntentRegistry(workspaceEditorIntents, makeWorkspaceEditorHandlers(state, bridge))
      let view = workspaceEditorView((yield* SubscriptionRef.get(state)).workspaceEditor)
      yield* registry.dispatch(pressIntent(view, "workspace-editor-close"))
      expect((yield* SubscriptionRef.get(state)).workspaceEditor.closeConfirmRef).toBe("src/index.ts")
      view = workspaceEditorView((yield* SubscriptionRef.get(state)).workspaceEditor)
      yield* registry.dispatch(pressIntent(view, "workspace-editor-close"))
      expect((yield* SubscriptionRef.get(state)).workspaceEditor.tabs).toEqual([])
    }))
  })

  test("watcher changes reopen the matching document and preserve a dirty draft as conflict", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const calls: unknown[] = []
      const bridge: WorkspaceDocumentBridge = {
        openWorkspaceDocument: async value => {
          calls.push(value)
          return {
            state: "available",
            document: document({ content: "external", revisionRef: "workspace.document.external" }),
          }
        },
        saveWorkspaceDocument: async () => ({ state: "unavailable", reason: "unavailable", message: "unused" }),
      }
      const dirty = withWorkspaceEditorEvent(readyState(), { type: "change", value: "local draft" })
      const state = yield* SubscriptionRef.make({ workspaceEditor: dirty })
      const registry = yield* makeIntentRegistry(workspaceEditorIntents, makeWorkspaceEditorHandlers(state, bridge))
      yield* registry.dispatch(resolveIntentRef(IntentRef("WorkspaceEditorExternalChangeReceived", StaticPayload({
        kind: "changed",
        pathRef: "src/index.ts",
        epoch: 2,
      })), null))
      expect(calls).toEqual([{ grantRef: "workspace.grant.editor", pathRef: "src/index.ts" }])
      expect((yield* SubscriptionRef.get(state)).workspaceEditor.tabs[0]).toMatchObject({
        phase: "conflict",
        draft: "local draft",
        externalDocument: { revisionRef: "workspace.document.external" },
      })
    }))
  })
})
