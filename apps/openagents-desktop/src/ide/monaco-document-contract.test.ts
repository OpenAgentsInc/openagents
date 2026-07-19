import { describe, expect, test } from "vite-plus/test"
import { readFileSync } from "node:fs"
import path from "node:path"
import { Effect, Exit, Schema } from "effect"

// Behavior oracle: openagents_desktop.ide_monaco_document_runtime.v1
import {
  IdeDocumentGeneration,
  IdeDocumentSequence,
  IdeEditorViewRef,
  IdeMonacoDocumentEventSchema,
  IdeMonacoModelVersion,
  IdeMonacoRuntimeResourceSnapshotSchema,
  IdeVimProjectionSchema,
  makeIdeDocumentRef,
} from "./monaco-document-contract.ts"
import {
  IdeMonacoAttachInputSchema,
  ideMonacoRuntimeResources,
} from "./monaco-runtime-loader.ts"
import { emptyIdeEditorSettings, resolveIdeMonacoEditorOptions } from "./workbench-contract.ts"

const appRoot = path.resolve(import.meta.dirname, "../..")

describe("IDE-03 schema-first Monaco document boundary", () => {
  test("opaque identities isolate grants and never encode a path", () => {
    const alpha = makeIdeDocumentRef("workspace.grant.alpha", 0)
    const beta = makeIdeDocumentRef("workspace.grant.beta", 0)
    const next = makeIdeDocumentRef("workspace.grant.alpha", 1)
    expect(alpha).not.toBe(beta)
    expect(alpha).not.toBe(next)
    expect(alpha).toMatch(/^ide\.document\.[a-z0-9]{12}\.0$/)
    expect(alpha).not.toContain("src")
    expect(alpha).not.toContain("grant")
  })

  test("events, Vim state, attach input, and resource receipts decode from Effect Schemas", async () => {
    const documentRef = makeIdeDocumentRef("workspace.grant.schema", 2)
    const generation = IdeDocumentGeneration.make(2)
    const event = IdeMonacoDocumentEventSchema.cases.Edit.make({
      documentRef,
      generation,
      sequence: IdeDocumentSequence.make(1),
      modelVersion: IdeMonacoModelVersion.make(2),
      value: "export const ok = true\n",
      changes: [{ offset: 0, length: 0, text: "export const ok = true\n" }],
    })
    expect(Schema.decodeUnknownSync(IdeMonacoDocumentEventSchema)(event)).toEqual(event)
    expect(IdeVimProjectionSchema.make({ enabled: true, mode: "normal", pending: null, count: 2 })).toEqual({
      enabled: true,
      mode: "normal",
      pending: null,
      count: 2,
    })
    expect(IdeMonacoAttachInputSchema.make({
      documentRef,
      generation,
      sequence: IdeDocumentSequence.make(1),
      documentVersion: IdeMonacoModelVersion.make(2),
      viewRef: IdeEditorViewRef.make("ide.view.schema.primary"),
      pathLabel: "src/schema.ts",
      language: "typescript",
      value: event.value,
      selection: { start: 0, end: 6 },
      selectionVersion: 1,
      wordWrap: false,
      minimap: false,
      vimEnabled: false,
      editorOptions: resolveIdeMonacoEditorOptions(emptyIdeEditorSettings()),
      readOnly: false,
      projectLanguage: null,
    })).toMatchObject({ documentRef, generation, language: "typescript" })
    const invalid = await Effect.runPromiseExit(Schema.decodeUnknownEffect(IdeMonacoDocumentEventSchema)({
      ...event,
      sequence: -1,
    }))
    expect(Exit.isFailure(invalid)).toBe(true)
    expect(Schema.decodeUnknownSync(IdeMonacoRuntimeResourceSnapshotSchema)(ideMonacoRuntimeResources())).toMatchObject({
      state: "idle",
      modelCount: 0,
      viewCount: 0,
      workerCount: 0,
    })
  })

  test("the production graph is a fixed offline island with no host authority", () => {
    const loader = readFileSync(path.join(appRoot, "src/ide/monaco-runtime-loader.ts"), "utf8")
    const runtime = readFileSync(path.join(appRoot, "src/ide/editor-runtime-entry.ts"), "utf8")
    const build = readFileSync(path.join(appRoot, "scripts/build.ts"), "utf8")
    const main = readFileSync(path.join(appRoot, "src/main.ts"), "utf8")
    const react = readFileSync(path.join(appRoot, "src/renderer/react-workspace-surfaces.tsx"), "utf8")

    expect(loader).toContain('openagents-app://renderer/ide-editor/editor.js')
    expect(loader).toContain('openagents-app://renderer/ide-editor/editor.css')
    expect(build).toContain('path.join(dist, "renderer", "ide-editor")')
    expect(main).toContain('asset.startsWith("ide-editor/")')
    expect(runtime).toContain('inmemory://openagents/')
    expect(runtime).toContain('monaco.editor.defineTheme("openagents-tokyo-night"')
    expect(runtime).toContain('monaco.editor.defineTheme("openagents-khala-editor"')
    expect(runtime).toContain('monaco.editor.setTheme("openagents-khala-editor")')
    expect(runtime).toContain("onDidBlurEditorWidget")
    expect(runtime).toContain("modelEntry.vimState")
    expect(runtime.indexOf('event.ctrlKey && key.toLocaleLowerCase() === "v"'))
      .toBeLessThan(runtime.indexOf('else if (key === "v")'))
    expect(runtime).not.toMatch(/from\s+["']node:/)
    expect(runtime).not.toContain("openWorkspaceDocument")
    expect(runtime).not.toContain("saveWorkspaceDocument")
    expect(react).toContain("<MonacoEditorHost")
    expect(react).not.toContain("<textarea")
    expect(react).not.toContain("oa-react-editor-find")
  })
})
