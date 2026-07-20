import { describe, expect, test } from "vite-plus/test"

import {
  buildEditorWorkContext,
  decodeEditorContextBinding,
  EDITOR_CONTEXT_BINDING_SCHEMA_LITERAL,
  makeEditorContextRegistry,
  type EditorContextBinding,
  type EditorContextIdentity,
} from "./editor-context-binding.ts"

const identity: EditorContextIdentity = {
  projectRef: "project.demo",
  rootRef: "root.demo",
  worktreeRef: "worktree.demo",
  generation: 7,
} as EditorContextIdentity

const bindingWith = (
  overrides: Partial<{
    identity: EditorContextIdentity
    items: ReadonlyArray<unknown>
    byteLimit: number
  }> = {},
): EditorContextBinding =>
  decodeEditorContextBinding({
    schema: EDITOR_CONTEXT_BINDING_SCHEMA_LITERAL,
    threadRef: "thread.demo",
    identity: overrides.identity ?? identity,
    byteLimit: overrides.byteLimit ?? 8_000,
    items: overrides.items ?? [
      { kind: "active_file", itemRef: "item.active", derived: false, byteLength: 400, truncated: false, redacted: false },
      { kind: "selection", itemRef: "item.selection", derived: false, byteLength: 60, truncated: false, redacted: false },
      { kind: "local_lexical", itemRef: "item.lexical", derived: false, byteLength: 120, truncated: false, redacted: false },
      { kind: "local_symbol", itemRef: "item.symbol", derived: true, byteLength: 40, truncated: false, redacted: false },
    ],
  })

const frozenNow = () => "2026-07-20T00:00:00.000Z"

describe("editor-context binding", () => {
  test("builds the effective manifest, binding project, worktree, generation, and items", () => {
    const built = buildEditorWorkContext(bindingWith(), identity, frozenNow)
    expect(built.ok).toBe(true)
    if (!built.ok) return
    expect(built.envelope.projectRef).toBe("project.demo")
    expect(built.envelope.worktreeRef).toBe("worktree.demo")
    expect(built.envelope.generation).toEqual({ state: "known", value: 7 })
    expect(built.envelope.items.map((item) => item.kind)).toEqual([
      "active_file",
      "selection",
      "local_lexical",
      "local_symbol",
    ])
    expect(built.envelope.totalByteLength).toBe(620)
    expect(built.envelope.truncated).toBe(false)
    expect(built.envelope.redacted).toBe(false)
    expect(built.envelope.threadRef).toBe("thread.demo")
  })

  test("local lexical and symbol context has no remote embedding dependency", () => {
    const built = buildEditorWorkContext(
      bindingWith({
        items: [
          { kind: "local_lexical", itemRef: "item.lexical", derived: false, byteLength: 100, truncated: false, redacted: false },
          { kind: "local_symbol", itemRef: "item.symbol", derived: true, byteLength: 30, truncated: false, redacted: false },
        ],
      }),
      identity,
      frozenNow,
    )
    expect(built.ok).toBe(true)
    if (!built.ok) return
    expect(built.noRemoteIndexDependency).toBe(true)
  })

  test("a remote semantic item flags a remote index dependency", () => {
    const built = buildEditorWorkContext(
      bindingWith({
        items: [
          { kind: "semantic_remote", itemRef: "item.remote", derived: true, byteLength: 100, truncated: false, redacted: false },
        ],
      }),
      identity,
      frozenNow,
    )
    expect(built.ok).toBe(true)
    if (!built.ok) return
    expect(built.noRemoteIndexDependency).toBe(false)
  })

  test("marks truncation when items exceed the byte budget", () => {
    const built = buildEditorWorkContext(
      bindingWith({
        byteLimit: 100,
        items: [
          { kind: "active_file", itemRef: "item.active", derived: false, byteLength: 400, truncated: true, redacted: false },
        ],
      }),
      identity,
      frozenNow,
    )
    expect(built.ok).toBe(true)
    if (!built.ok) return
    expect(built.envelope.truncated).toBe(true)
  })

  test("refuses when there is no active editor", () => {
    const built = buildEditorWorkContext(bindingWith(), null, frozenNow)
    expect(built).toEqual({ ok: false, reason: "no_active_editor" })
  })

  test("refuses context from another project", () => {
    const built = buildEditorWorkContext(
      bindingWith(),
      { ...identity, projectRef: "project.other" } as EditorContextIdentity,
      frozenNow,
    )
    expect(built).toEqual({ ok: false, reason: "project_mismatch" })
  })

  test("refuses context from another root", () => {
    const built = buildEditorWorkContext(
      bindingWith(),
      { ...identity, rootRef: "root.other" } as EditorContextIdentity,
      frozenNow,
    )
    expect(built).toEqual({ ok: false, reason: "root_mismatch" })
  })

  test("refuses context from another worktree", () => {
    const built = buildEditorWorkContext(
      bindingWith(),
      { ...identity, worktreeRef: "worktree.other" } as EditorContextIdentity,
      frozenNow,
    )
    expect(built).toEqual({ ok: false, reason: "worktree_mismatch" })
  })

  test("refuses a stale generation so a stale candidate cannot apply", () => {
    const built = buildEditorWorkContext(
      bindingWith({ identity: { ...identity, generation: 6 } as EditorContextIdentity }),
      identity,
      frozenNow,
    )
    expect(built).toEqual({ ok: false, reason: "stale_generation" })
  })

  test("registry stores and clears the binding and expectation", () => {
    const registry = makeEditorContextRegistry()
    expect(registry.get("thread.demo")).toBeNull()
    expect(registry.expectation()).toBeNull()
    const binding = bindingWith()
    registry.set(binding)
    registry.setExpectation(identity)
    expect(registry.get("thread.demo")).toBe(binding)
    expect(registry.expectation()).toEqual(identity)
    registry.clear("thread.demo")
    expect(registry.get("thread.demo")).toBeNull()
  })
})
