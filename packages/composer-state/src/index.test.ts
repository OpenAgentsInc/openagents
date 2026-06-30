import { describe, expect, test } from "bun:test"

import {
  applyComposerTransaction,
  composerAttachmentId,
  composerBlockId,
  emptyComposerState,
  moveComposerAttachmentSelection,
  offerComposerLargeTextPaste,
  parseComposerMarkdown,
  redoComposerState,
  retryComposerAttachmentTransaction,
  resolveComposerKeyBinding,
  runComposerInputRules,
  serializeComposerMarkdown,
  stageComposerDroppedFiles,
  stageComposerPastedFiles,
  setComposerAttachmentStatusTransaction,
  undoComposerState,
  type ComposerAttachment,
  type ComposerState,
  type ComposerTransaction,
} from "./index"

const tx = (steps: ComposerTransaction["steps"]): ComposerTransaction => ({
  steps,
  meta: { source: "program", time: 1 },
})

const apply = (
  state: ComposerState,
  transaction: ComposerTransaction,
): ComposerState => {
  const result = applyComposerTransaction(state, transaction)
  expect(result.ok).toBe(true)
  if (!result.ok) throw new Error(result.error.message)
  return result.state
}

const firstText = (state: ComposerState): string => {
  const block = state.doc.blocks[0]
  if (block === undefined || block.kind === "attachmentRef" || block.kind === "list") {
    return ""
  }
  return block.text
}

describe("composer state core", () => {
  test("applies insert, delete, replace, selection mapping, undo, and redo", () => {
    const blockId = composerBlockId("block-1")
    const start = emptyComposerState()
    const inserted = apply(
      start,
      tx([{ _tag: "InsertText", at: { blockId, offset: 0 }, text: "hello world" }]),
    )
    expect(firstText(inserted)).toBe("hello world")
    expect(inserted.selection.head.offset).toBe(11)

    const replaced = apply(
      inserted,
      tx([
        {
          _tag: "ReplaceRange",
          range: {
            anchor: { blockId, offset: 6 },
            head: { blockId, offset: 11 },
          },
          text: "Khala",
        },
      ]),
    )
    expect(firstText(replaced)).toBe("hello Khala")
    expect(replaced.selection.head.offset).toBe(11)

    const deleted = apply(
      replaced,
      tx([
        {
          _tag: "DeleteRange",
          range: {
            anchor: { blockId, offset: 0 },
            head: { blockId, offset: 6 },
          },
        },
      ]),
    )
    expect(firstText(deleted)).toBe("Khala")
    expect(deleted.history.done).toHaveLength(3)

    const undone = undoComposerState(deleted)
    expect(undone.ok).toBe(true)
    if (!undone.ok) throw new Error(undone.error.message)
    expect(firstText(undone.state)).toBe("hello Khala")
    expect(undone.state.history.undone).toHaveLength(1)

    const redone = redoComposerState(undone.state)
    expect(redone.ok).toBe(true)
    if (!redone.ok) throw new Error(redone.error.message)
    expect(firstText(redone.state)).toBe("Khala")
  })

  test("inserts, removes, and restores attachment refs through history", () => {
    const attachment: ComposerAttachment = {
      id: composerAttachmentId("att-1"),
      kind: "image",
      name: "screen.png",
      mime: "image/png",
      sizeBytes: 1024,
      status: "staged",
      dimensions: { width: 640, height: 480 },
    }
    const inserted = apply(
      emptyComposerState(),
      tx([{ _tag: "InsertAttachment", attachment, at: { index: 1 } }]),
    )
    expect(inserted.doc.attachments).toHaveLength(1)
    expect(inserted.doc.blocks.at(1)?.kind).toBe("attachmentRef")

    const removed = apply(
      inserted,
      tx([{ _tag: "RemoveAttachment", attachmentId: attachment.id }]),
    )
    expect(removed.doc.attachments).toHaveLength(0)
    expect(removed.doc.blocks.some((block) => block.kind === "attachmentRef")).toBe(false)

    const undo = undoComposerState(removed)
    expect(undo.ok).toBe(true)
    if (!undo.ok) throw new Error(undo.error.message)
    expect(undo.state.doc.attachments).toHaveLength(1)
    expect(undo.state.doc.blocks.at(1)?.kind).toBe("attachmentRef")
  })

  test("stages pasted images as metadata plus deferred thumbnail work", () => {
    const staged = stageComposerPastedFiles(
      [
        {
          name: "screen.png",
          type: "image/png",
          size: 2048,
        },
      ],
      { at: { index: 1 }, idPrefix: "paste-image" },
    )
    expect(staged.attachments).toEqual([
      expect.objectContaining({
        id: composerAttachmentId("paste-image-1"),
        kind: "image",
        name: "screen.png",
        mime: "image/png",
        sizeBytes: 2048,
        source: "paste",
        status: "staged",
      }),
    ])
    expect(staged.deferredTasks.map((task) => task.kind)).toEqual([
      "create_image_thumbnail",
      "extract_image_dimensions",
    ])

    const state = apply(emptyComposerState(), staged.transaction)
    expect(state.doc.attachments[0]?.status).toBe("staged")
    expect(state.doc.blocks.at(1)?.kind).toBe("attachmentRef")
  })

  test("stages dropped files without implying upload success", () => {
    const staged = stageComposerDroppedFiles(
      [
        {
          name: "notes.md",
          type: "text/markdown",
          size: 512,
          contentRef: "local-file:notes",
        },
      ],
      { idPrefix: "drop-file" },
    )
    expect(staged.attachments[0]).toMatchObject({
      id: composerAttachmentId("drop-file-1"),
      kind: "text",
      status: "staged",
      source: "drop",
      contentRef: "local-file:notes",
    })
    expect(staged.attachments[0]?.status).not.toBe("ready")
    expect(staged.deferredTasks.map((task) => task.kind)).toEqual([
      "count_text_bytes",
      "estimate_text_tokens",
    ])
  })

  test("offers large pasted text as an attachment instead of rewriting source text", () => {
    const largeText = "x".repeat(100_000)
    const offer = offerComposerLargeTextPaste(largeText, {
      id: composerAttachmentId("paste-large-text"),
      thresholdBytes: 16_000,
    })
    expect(offer.offered).toBe(true)
    expect(offer.textBytes).toBe(100_000)
    expect(offer.attachment).toMatchObject({
      id: composerAttachmentId("paste-large-text"),
      kind: "text",
      mime: "text/plain",
      sizeBytes: 100_000,
      status: "staged",
      source: "paste",
    })
    expect(offer.transaction?.steps).toHaveLength(1)

    const state = apply(emptyComposerState(), offer.transaction!)
    expect(firstText(state)).toBe("")
    expect(serializeComposerMarkdown(state.doc)).toContain(
      "[attachment:pasted-text.txt]",
    )
  })

  test("does not offer small pasted text as an attachment", () => {
    const offer = offerComposerLargeTextPaste("small", { thresholdBytes: 16_000 })
    expect(offer.offered).toBe(false)
    expect(offer.transaction).toBe(null)
    expect(offer.deferredTasks).toEqual([])
  })

  test("updates attachment status for error, retry, ready, remove, and undo", () => {
    const staged = stageComposerPastedFiles(
      [{ name: "failure.txt", type: "text/plain", size: 12 }],
      { idPrefix: "retry" },
    )
    const inserted = apply(emptyComposerState(), staged.transaction)
    const attachmentId = composerAttachmentId("retry-1")
    const errorTx = setComposerAttachmentStatusTransaction(inserted, attachmentId, {
      status: "error",
      errorText: "Upload failed",
    })
    expect(errorTx).not.toBe(null)
    const errored = apply(inserted, errorTx!)
    expect(errored.doc.attachments[0]).toMatchObject({
      status: "error",
      errorText: "Upload failed",
    })

    const retryTx = retryComposerAttachmentTransaction(errored, attachmentId, 2)
    expect(retryTx).not.toBe(null)
    const retried = apply(errored, retryTx!)
    expect(retried.doc.attachments[0]).toMatchObject({ status: "staged" })
    expect(retried.doc.attachments[0]?.errorText).toBeUndefined()

    const readyTx = setComposerAttachmentStatusTransaction(retried, attachmentId, {
      status: "ready",
      digest: "sha256:abcd",
    })
    const ready = apply(retried, readyTx!)
    expect(ready.doc.attachments[0]).toMatchObject({
      status: "ready",
      digest: "sha256:abcd",
    })

    const removed = apply(
      ready,
      tx([{ _tag: "RemoveAttachment", attachmentId }]),
    )
    expect(removed.doc.attachments).toHaveLength(0)
    const undo = undoComposerState(removed)
    expect(undo.ok).toBe(true)
    if (!undo.ok) throw new Error(undo.error.message)
    expect(undo.state.doc.attachments[0]).toMatchObject({
      status: "ready",
      digest: "sha256:abcd",
    })
  })

  test("moves keyboard selection across attachment refs", () => {
    const staged = stageComposerDroppedFiles(
      [
        { name: "one.txt", type: "text/plain", size: 1 },
        { name: "two.png", type: "image/png", size: 2 },
      ],
      { at: { index: 1 }, idPrefix: "nav" },
    )
    const state = apply(emptyComposerState(), staged.transaction)
    const first = moveComposerAttachmentSelection(state, "first")
    expect(first.selection.selectedAttachmentId).toBe(composerAttachmentId("nav-1"))
    const next = moveComposerAttachmentSelection(first, "next")
    expect(next.selection.selectedAttachmentId).toBe(composerAttachmentId("nav-2"))
    const previous = moveComposerAttachmentSelection(next, "previous")
    expect(previous.selection.selectedAttachmentId).toBe(
      composerAttachmentId("nav-1"),
    )
    const cleared = moveComposerAttachmentSelection(previous, "clear")
    expect(cleared.selection.selectedAttachmentId).toBeUndefined()
    expect(resolveComposerKeyBinding({ key: "ArrowRight", altKey: true })).toBe(
      "select_next_attachment",
    )
  })

  test("stores resize as a transaction and can undo it", () => {
    const resized = apply(
      emptyComposerState(),
      tx([{ _tag: "ResizeComposer", heightPx: 320 }]),
    )
    expect(resized.view.heightPx).toBe(320)
    const undo = undoComposerState(resized)
    expect(undo.ok).toBe(true)
    if (!undo.ok) throw new Error(undo.error.message)
    expect(undo.state.view.heightPx).toBe(0)
  })

  test("resolves explicit composer shortcuts without taking copy/paste/select-all", () => {
    expect(resolveComposerKeyBinding({ key: "Enter", metaKey: true })).toBe("submit")
    expect(resolveComposerKeyBinding({ key: "Enter", shiftKey: true })).toBe(
      "insert_newline",
    )
    expect(resolveComposerKeyBinding({ key: "c", metaKey: true })).toBe(null)
    expect(resolveComposerKeyBinding({ key: "v", metaKey: true })).toBe(null)
    expect(resolveComposerKeyBinding({ key: "a", metaKey: true })).toBe(null)
  })

  test("runs Markdown input rules as typed transactions", () => {
    const blockId = composerBlockId("block-1")
    const state = apply(
      emptyComposerState(),
      tx([{ _tag: "InsertText", at: { blockId, offset: 0 }, text: "```ts" }]),
    )
    const rule = runComposerInputRules(state, "s")
    expect(rule?.steps[0]?._tag).toBe("SetBlockKind")
    expect(rule?.steps[0]).toMatchObject({ kind: "code", language: "ts" })
  })

  test("round-trips the v1 Markdown subset and preserves unsupported text", () => {
    const markdown = [
      "hello **raw** world",
      "",
      "> quoted",
      "> line",
      "",
      "- one",
      "- two",
      "",
      "```ts",
      "const answer = 42",
      "```",
      "",
      "<custom-widget data-x=\"1\">keep me</custom-widget>",
    ].join("\n")
    const doc = parseComposerMarkdown(markdown)
    const serialized = serializeComposerMarkdown(doc)
    expect(serialized).toContain("hello **raw** world")
    expect(serialized).toContain("> quoted\n> line")
    expect(serialized).toContain("- one\n- two")
    expect(serialized).toContain("```ts\nconst answer = 42\n```")
    expect(serialized).toContain("<custom-widget data-x=\"1\">keep me</custom-widget>")
  })
})
