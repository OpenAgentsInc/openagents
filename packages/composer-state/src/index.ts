import { Schema as S } from "effect"

export const COMPOSER_SCHEMA_VERSION = "openagents.composer.v1" as const

export const ComposerSchemaVersion = S.Literal(COMPOSER_SCHEMA_VERSION)
export type ComposerSchemaVersion = typeof ComposerSchemaVersion.Type

export const ComposerBlockId = S.String.pipe(S.brand("ComposerBlockId"))
export type ComposerBlockId = typeof ComposerBlockId.Type

export const ComposerAttachmentId = S.String.pipe(
  S.brand("ComposerAttachmentId"),
)
export type ComposerAttachmentId = typeof ComposerAttachmentId.Type

export const ComposerBlockKind = S.Literals([
  "paragraph",
  "code",
  "quote",
  "list",
  "attachmentRef",
])
export type ComposerBlockKind = typeof ComposerBlockKind.Type

export const ComposerTextBlockKind = S.Literals([
  "paragraph",
  "code",
  "quote",
])
export type ComposerTextBlockKind = typeof ComposerTextBlockKind.Type

export const ComposerInlineMarkKind = S.Literals([
  "strong",
  "emphasis",
  "code",
  "link",
  "mention",
  "toolRef",
])
export type ComposerInlineMarkKind = typeof ComposerInlineMarkKind.Type

export const ComposerInlineMark = S.Struct({
  kind: ComposerInlineMarkKind,
  from: S.Number,
  to: S.Number,
  href: S.optional(S.String),
  ref: S.optional(S.String),
})
export type ComposerInlineMark = typeof ComposerInlineMark.Type

export const ComposerParagraphBlock = S.Struct({
  id: ComposerBlockId,
  kind: S.Literal("paragraph"),
  text: S.String,
  marks: S.Array(ComposerInlineMark),
})
export type ComposerParagraphBlock = typeof ComposerParagraphBlock.Type

export const ComposerCodeBlock = S.Struct({
  id: ComposerBlockId,
  kind: S.Literal("code"),
  text: S.String,
  language: S.optional(S.String),
})
export type ComposerCodeBlock = typeof ComposerCodeBlock.Type

export const ComposerQuoteBlock = S.Struct({
  id: ComposerBlockId,
  kind: S.Literal("quote"),
  text: S.String,
})
export type ComposerQuoteBlock = typeof ComposerQuoteBlock.Type

export const ComposerListBlock = S.Struct({
  id: ComposerBlockId,
  kind: S.Literal("list"),
  ordered: S.Boolean,
  items: S.Array(S.String),
})
export type ComposerListBlock = typeof ComposerListBlock.Type

export const ComposerAttachmentRefBlock = S.Struct({
  id: ComposerBlockId,
  kind: S.Literal("attachmentRef"),
  attachmentId: ComposerAttachmentId,
})
export type ComposerAttachmentRefBlock = typeof ComposerAttachmentRefBlock.Type

export const ComposerBlock = S.Union([
  ComposerParagraphBlock,
  ComposerCodeBlock,
  ComposerQuoteBlock,
  ComposerListBlock,
  ComposerAttachmentRefBlock,
])
export type ComposerBlock = typeof ComposerBlock.Type

export const ComposerAttachmentKind = S.Literals([
  "image",
  "file",
  "text",
  "snippet",
])
export type ComposerAttachmentKind = typeof ComposerAttachmentKind.Type

export const ComposerAttachmentStatus = S.Literals([
  "staged",
  "uploading",
  "ready",
  "error",
])
export type ComposerAttachmentStatus = typeof ComposerAttachmentStatus.Type

export const ComposerAttachmentDimensions = S.Struct({
  width: S.Number,
  height: S.Number,
})
export type ComposerAttachmentDimensions =
  typeof ComposerAttachmentDimensions.Type

export const ComposerAttachment = S.Struct({
  id: ComposerAttachmentId,
  kind: ComposerAttachmentKind,
  name: S.String,
  mime: S.String,
  sizeBytes: S.Number,
  digest: S.optional(S.String),
  previewUrl: S.optional(S.String),
  dimensions: S.optional(ComposerAttachmentDimensions),
  status: ComposerAttachmentStatus,
  errorText: S.optional(S.String),
})
export type ComposerAttachment = typeof ComposerAttachment.Type

export const ComposerDoc = S.Struct({
  schemaVersion: ComposerSchemaVersion,
  blocks: S.Array(ComposerBlock),
  attachments: S.Array(ComposerAttachment),
})
export type ComposerDoc = typeof ComposerDoc.Type

export const ComposerTextPosition = S.Struct({
  blockId: ComposerBlockId,
  offset: S.Number,
})
export type ComposerTextPosition = typeof ComposerTextPosition.Type

export const ComposerTextRange = S.Struct({
  anchor: ComposerTextPosition,
  head: ComposerTextPosition,
})
export type ComposerTextRange = typeof ComposerTextRange.Type

export const ComposerBlockPosition = S.Struct({
  index: S.optional(S.Number),
  beforeBlockId: S.optional(ComposerBlockId),
  afterBlockId: S.optional(ComposerBlockId),
})
export type ComposerBlockPosition = typeof ComposerBlockPosition.Type

export const ComposerSelection = S.Struct({
  anchor: ComposerTextPosition,
  head: ComposerTextPosition,
  selectedAttachmentId: S.optional(ComposerAttachmentId),
})
export type ComposerSelection = typeof ComposerSelection.Type

export const ComposerInsertTextStep = S.TaggedStruct("InsertText", {
  at: ComposerTextPosition,
  text: S.String,
})
export type ComposerInsertTextStep = typeof ComposerInsertTextStep.Type

export const ComposerDeleteRangeStep = S.TaggedStruct("DeleteRange", {
  range: ComposerTextRange,
})
export type ComposerDeleteRangeStep = typeof ComposerDeleteRangeStep.Type

export const ComposerReplaceRangeStep = S.TaggedStruct("ReplaceRange", {
  range: ComposerTextRange,
  text: S.String,
})
export type ComposerReplaceRangeStep = typeof ComposerReplaceRangeStep.Type

export const ComposerSetBlockKindStep = S.TaggedStruct("SetBlockKind", {
  blockId: ComposerBlockId,
  kind: ComposerTextBlockKind,
  language: S.optional(S.String),
})
export type ComposerSetBlockKindStep = typeof ComposerSetBlockKindStep.Type

export const ComposerInsertAttachmentStep = S.TaggedStruct("InsertAttachment", {
  attachment: ComposerAttachment,
  at: S.optional(ComposerBlockPosition),
})
export type ComposerInsertAttachmentStep =
  typeof ComposerInsertAttachmentStep.Type

export const ComposerRemoveAttachmentStep = S.TaggedStruct("RemoveAttachment", {
  attachmentId: ComposerAttachmentId,
})
export type ComposerRemoveAttachmentStep =
  typeof ComposerRemoveAttachmentStep.Type

export const ComposerResizeComposerStep = S.TaggedStruct("ResizeComposer", {
  heightPx: S.Number,
})
export type ComposerResizeComposerStep = typeof ComposerResizeComposerStep.Type

export const ComposerStep = S.Union([
  ComposerInsertTextStep,
  ComposerDeleteRangeStep,
  ComposerReplaceRangeStep,
  ComposerSetBlockKindStep,
  ComposerInsertAttachmentStep,
  ComposerRemoveAttachmentStep,
  ComposerResizeComposerStep,
])
export type ComposerStep = typeof ComposerStep.Type

export const ComposerTransactionSource = S.Literals([
  "input",
  "paste",
  "drop",
  "keymap",
  "program",
  "submit",
  "undo",
  "redo",
])
export type ComposerTransactionSource = typeof ComposerTransactionSource.Type

export const ComposerTransactionMeta = S.Struct({
  source: ComposerTransactionSource,
  time: S.Number,
  addToHistory: S.optional(S.Boolean),
})
export type ComposerTransactionMeta = typeof ComposerTransactionMeta.Type

export const ComposerTransaction = S.Struct({
  steps: S.Array(ComposerStep),
  selection: S.optional(ComposerSelection),
  meta: ComposerTransactionMeta,
})
export type ComposerTransaction = typeof ComposerTransaction.Type

export const ComposerViewState = S.Struct({
  heightPx: S.optional(S.Number),
  expanded: S.optional(S.Boolean),
  preview: S.optional(S.Boolean),
})
export type ComposerViewState = typeof ComposerViewState.Type

export type ComposerHistoryEntry = Readonly<{
  undo: ComposerTransaction
  redo: ComposerTransaction
}>

export type ComposerHistory = Readonly<{
  done: ReadonlyArray<ComposerHistoryEntry>
  undone: ReadonlyArray<ComposerHistoryEntry>
}>

export type ComposerState = Readonly<{
  doc: ComposerDoc
  selection: ComposerSelection
  view: ComposerViewState
  history: ComposerHistory
}>

export type ComposerReducerErrorTag =
  | "block_not_found"
  | "attachment_not_found"
  | "invalid_range"
  | "unsupported_block"

export type ComposerReducerError = Readonly<{
  _tag: ComposerReducerErrorTag
  message: string
}>

export type ComposerApplyStepSuccess = Readonly<{
  ok: true
  state: ComposerState
  inverse: ComposerStep
}>

export type ComposerApplyTransactionSuccess = Readonly<{
  ok: true
  state: ComposerState
  inverseSteps: ReadonlyArray<ComposerStep>
}>

export type ComposerApplyFailure = Readonly<{
  ok: false
  error: ComposerReducerError
}>

export type ComposerApplyStepResult =
  | ComposerApplyStepSuccess
  | ComposerApplyFailure

export type ComposerApplyTransactionResult =
  | ComposerApplyTransactionSuccess
  | ComposerApplyFailure

const err = (
  _tag: ComposerReducerErrorTag,
  message: string,
): ComposerApplyFailure => ({ ok: false, error: { _tag, message } })

export const composerBlockId = (id: string): ComposerBlockId =>
  id as ComposerBlockId

export const composerAttachmentId = (id: string): ComposerAttachmentId =>
  id as ComposerAttachmentId

export const createParagraphBlock = (
  id: string,
  text = "",
): ComposerParagraphBlock => ({
  id: composerBlockId(id),
  kind: "paragraph",
  text,
  marks: [],
})

export const emptyComposerDoc = (): ComposerDoc => ({
  schemaVersion: COMPOSER_SCHEMA_VERSION,
  blocks: [createParagraphBlock("block-1")],
  attachments: [],
})

export const emptyComposerSelection = (
  blockId: ComposerBlockId = composerBlockId("block-1"),
): ComposerSelection => ({
  anchor: { blockId, offset: 0 },
  head: { blockId, offset: 0 },
})

export const emptyComposerState = (): ComposerState => ({
  doc: emptyComposerDoc(),
  selection: emptyComposerSelection(),
  view: {},
  history: { done: [], undone: [] },
})

const isTextBlock = (
  block: ComposerBlock,
): block is ComposerParagraphBlock | ComposerCodeBlock | ComposerQuoteBlock =>
  block.kind === "paragraph" || block.kind === "code" || block.kind === "quote"

const blockIndexById = (
  blocks: ReadonlyArray<ComposerBlock>,
  blockId: ComposerBlockId,
): number => blocks.findIndex((block) => block.id === blockId)

const getTextBlock = (
  doc: ComposerDoc,
  blockId: ComposerBlockId,
):
  | Readonly<{ block: ComposerParagraphBlock | ComposerCodeBlock | ComposerQuoteBlock; index: number }>
  | ComposerApplyFailure => {
  const index = blockIndexById(doc.blocks, blockId)
  if (index < 0) return err("block_not_found", `Block ${blockId} was not found`)
  const block = doc.blocks[index]
  if (block === undefined || !isTextBlock(block)) {
    return err("unsupported_block", `Block ${blockId} does not contain text`)
  }
  return { block, index }
}

const withText = (
  block: ComposerParagraphBlock | ComposerCodeBlock | ComposerQuoteBlock,
  text: string,
): ComposerParagraphBlock | ComposerCodeBlock | ComposerQuoteBlock => {
  if (block.kind === "paragraph") return { ...block, text }
  if (block.kind === "code") return { ...block, text }
  return { ...block, text }
}

const replaceBlock = (
  blocks: ReadonlyArray<ComposerBlock>,
  index: number,
  block: ComposerBlock,
): ReadonlyArray<ComposerBlock> =>
  blocks.map((candidate, candidateIndex) =>
    candidateIndex === index ? block : candidate,
  )

const clampOffset = (offset: number, text: string): number =>
  Math.max(0, Math.min(text.length, Math.trunc(offset)))

const orderedRange = (
  range: ComposerTextRange,
  text: string,
):
  | Readonly<{ from: number; to: number; blockId: ComposerBlockId }>
  | ComposerApplyFailure => {
  if (range.anchor.blockId !== range.head.blockId) {
    return err("invalid_range", "Cross-block ranges are not supported in v1")
  }
  const anchor = clampOffset(range.anchor.offset, text)
  const head = clampOffset(range.head.offset, text)
  return {
    from: Math.min(anchor, head),
    to: Math.max(anchor, head),
    blockId: range.anchor.blockId,
  }
}

const makeRange = (
  blockId: ComposerBlockId,
  from: number,
  to: number,
): ComposerTextRange => ({
  anchor: { blockId, offset: from },
  head: { blockId, offset: to },
})

const mapPositionThroughStep = (
  position: ComposerTextPosition,
  step: ComposerStep,
): ComposerTextPosition => {
  if (
    step._tag !== "InsertText" &&
    step._tag !== "DeleteRange" &&
    step._tag !== "ReplaceRange"
  ) {
    return position
  }

  const blockId =
    step._tag === "InsertText" ? step.at.blockId : step.range.anchor.blockId
  if (position.blockId !== blockId) return position

  if (step._tag === "InsertText") {
    const at = Math.max(0, Math.trunc(step.at.offset))
    return position.offset < at
      ? position
      : { ...position, offset: position.offset + step.text.length }
  }

  const anchor = Math.trunc(step.range.anchor.offset)
  const head = Math.trunc(step.range.head.offset)
  const from = Math.min(anchor, head)
  const to = Math.max(anchor, head)
  const inserted = step._tag === "ReplaceRange" ? step.text.length : 0
  if (position.offset <= from) return position
  if (position.offset <= to) return { ...position, offset: from + inserted }
  return { ...position, offset: position.offset - (to - from) + inserted }
}

export const mapSelectionThroughStep = (
  selection: ComposerSelection,
  step: ComposerStep,
): ComposerSelection => {
  const anchor = mapPositionThroughStep(selection.anchor, step)
  const head = mapPositionThroughStep(selection.head, step)
  return selection.selectedAttachmentId === undefined
    ? { anchor, head }
    : { anchor, head, selectedAttachmentId: selection.selectedAttachmentId }
}

const setSelectionAfterText = (
  state: ComposerState,
  blockId: ComposerBlockId,
  offset: number,
): ComposerState => ({
  ...state,
  selection: {
    anchor: { blockId, offset },
    head: { blockId, offset },
  },
})

const applyInsertText = (
  state: ComposerState,
  step: ComposerInsertTextStep,
): ComposerApplyStepResult => {
  const found = getTextBlock(state.doc, step.at.blockId)
  if ("ok" in found) return found
  const offset = clampOffset(step.at.offset, found.block.text)
  const text = `${found.block.text.slice(0, offset)}${step.text}${found.block.text.slice(offset)}`
  const block = withText(found.block, text)
  const doc = {
    ...state.doc,
    blocks: replaceBlock(state.doc.blocks, found.index, block),
  }
  const next = setSelectionAfterText({ ...state, doc }, step.at.blockId, offset + step.text.length)
  return {
    ok: true,
    state: next,
    inverse: {
      _tag: "DeleteRange",
      range: makeRange(step.at.blockId, offset, offset + step.text.length),
    },
  }
}

const applyDeleteRange = (
  state: ComposerState,
  step: ComposerDeleteRangeStep,
): ComposerApplyStepResult => {
  const found = getTextBlock(state.doc, step.range.anchor.blockId)
  if ("ok" in found) return found
  const range = orderedRange(step.range, found.block.text)
  if ("ok" in range) return range
  const removed = found.block.text.slice(range.from, range.to)
  const text = `${found.block.text.slice(0, range.from)}${found.block.text.slice(range.to)}`
  const block = withText(found.block, text)
  const doc = {
    ...state.doc,
    blocks: replaceBlock(state.doc.blocks, found.index, block),
  }
  const next = setSelectionAfterText({ ...state, doc }, range.blockId, range.from)
  return {
    ok: true,
    state: next,
    inverse: { _tag: "InsertText", at: { blockId: range.blockId, offset: range.from }, text: removed },
  }
}

const applyReplaceRange = (
  state: ComposerState,
  step: ComposerReplaceRangeStep,
): ComposerApplyStepResult => {
  const found = getTextBlock(state.doc, step.range.anchor.blockId)
  if ("ok" in found) return found
  const range = orderedRange(step.range, found.block.text)
  if ("ok" in range) return range
  const removed = found.block.text.slice(range.from, range.to)
  const text = `${found.block.text.slice(0, range.from)}${step.text}${found.block.text.slice(range.to)}`
  const block = withText(found.block, text)
  const doc = {
    ...state.doc,
    blocks: replaceBlock(state.doc.blocks, found.index, block),
  }
  const next = setSelectionAfterText({ ...state, doc }, range.blockId, range.from + step.text.length)
  return {
    ok: true,
    state: next,
    inverse: {
      _tag: "ReplaceRange",
      range: makeRange(range.blockId, range.from, range.from + step.text.length),
      text: removed,
    },
  }
}

const applySetBlockKind = (
  state: ComposerState,
  step: ComposerSetBlockKindStep,
): ComposerApplyStepResult => {
  const found = getTextBlock(state.doc, step.blockId)
  if ("ok" in found) return found
  const previous = found.block
  const nextBlock: ComposerParagraphBlock | ComposerCodeBlock | ComposerQuoteBlock =
    step.kind === "paragraph"
      ? { id: previous.id, kind: "paragraph", text: previous.text, marks: [] }
      : step.kind === "code"
        ? {
            id: previous.id,
            kind: "code",
            text: previous.text,
            ...(step.language === undefined ? {} : { language: step.language }),
          }
        : { id: previous.id, kind: "quote", text: previous.text }
  const doc = {
    ...state.doc,
    blocks: replaceBlock(state.doc.blocks, found.index, nextBlock),
  }
  const inverse: ComposerSetBlockKindStep =
    previous.kind === "code"
      ? {
          _tag: "SetBlockKind",
          blockId: previous.id,
          kind: "code",
          ...(previous.language === undefined ? {} : { language: previous.language }),
        }
      : { _tag: "SetBlockKind", blockId: previous.id, kind: previous.kind }
  return { ok: true, state: { ...state, doc }, inverse }
}

const insertIndexFor = (
  blocks: ReadonlyArray<ComposerBlock>,
  position: ComposerBlockPosition | undefined,
): number => {
  if (position === undefined) return blocks.length
  if (position.index !== undefined) {
    return Math.max(0, Math.min(blocks.length, Math.trunc(position.index)))
  }
  if (position.beforeBlockId !== undefined) {
    const index = blockIndexById(blocks, position.beforeBlockId)
    return index < 0 ? blocks.length : index
  }
  if (position.afterBlockId !== undefined) {
    const index = blockIndexById(blocks, position.afterBlockId)
    return index < 0 ? blocks.length : index + 1
  }
  return blocks.length
}

const applyInsertAttachment = (
  state: ComposerState,
  step: ComposerInsertAttachmentStep,
): ComposerApplyStepResult => {
  const attachmentId = step.attachment.id
  const withoutExistingAttachment = state.doc.attachments.filter(
    (attachment) => attachment.id !== attachmentId,
  )
  const withoutExistingRefs = state.doc.blocks.filter(
    (block) => block.kind !== "attachmentRef" || block.attachmentId !== attachmentId,
  )
  const index = insertIndexFor(withoutExistingRefs, step.at)
  const refBlock: ComposerAttachmentRefBlock = {
    id: composerBlockId(`attachment-${attachmentId}`),
    kind: "attachmentRef",
    attachmentId,
  }
  const blocks = [
    ...withoutExistingRefs.slice(0, index),
    refBlock,
    ...withoutExistingRefs.slice(index),
  ]
  return {
    ok: true,
    state: {
      ...state,
      doc: {
        ...state.doc,
        attachments: [...withoutExistingAttachment, step.attachment],
        blocks,
      },
    },
    inverse: { _tag: "RemoveAttachment", attachmentId },
  }
}

const applyRemoveAttachment = (
  state: ComposerState,
  step: ComposerRemoveAttachmentStep,
): ComposerApplyStepResult => {
  const attachment = state.doc.attachments.find(
    (candidate) => candidate.id === step.attachmentId,
  )
  if (attachment === undefined) {
    return err("attachment_not_found", `Attachment ${step.attachmentId} was not found`)
  }
  const firstRefIndex = state.doc.blocks.findIndex(
    (block) =>
      block.kind === "attachmentRef" && block.attachmentId === step.attachmentId,
  )
  const blocks = state.doc.blocks.filter(
    (block) => block.kind !== "attachmentRef" || block.attachmentId !== step.attachmentId,
  )
  const attachments = state.doc.attachments.filter(
    (candidate) => candidate.id !== step.attachmentId,
  )
  const at =
    firstRefIndex < 0
      ? undefined
      : { index: Math.max(0, Math.min(blocks.length, firstRefIndex)) }
  return {
    ok: true,
    state: { ...state, doc: { ...state.doc, blocks, attachments } },
    inverse: {
      _tag: "InsertAttachment",
      attachment,
      ...(at === undefined ? {} : { at }),
    },
  }
}

const applyResizeComposer = (
  state: ComposerState,
  step: ComposerResizeComposerStep,
): ComposerApplyStepResult => {
  const previous = state.view.heightPx ?? 0
  return {
    ok: true,
    state: {
      ...state,
      view: { ...state.view, heightPx: Math.max(0, Math.trunc(step.heightPx)) },
    },
    inverse: { _tag: "ResizeComposer", heightPx: previous },
  }
}

export const applyComposerStep = (
  state: ComposerState,
  step: ComposerStep,
): ComposerApplyStepResult => {
  switch (step._tag) {
    case "InsertText":
      return applyInsertText(state, step)
    case "DeleteRange":
      return applyDeleteRange(state, step)
    case "ReplaceRange":
      return applyReplaceRange(state, step)
    case "SetBlockKind":
      return applySetBlockKind(state, step)
    case "InsertAttachment":
      return applyInsertAttachment(state, step)
    case "RemoveAttachment":
      return applyRemoveAttachment(state, step)
    case "ResizeComposer":
      return applyResizeComposer(state, step)
  }
}

export const applyComposerTransaction = (
  state: ComposerState,
  transaction: ComposerTransaction,
): ComposerApplyTransactionResult => {
  let next = state
  const inverseSteps: ComposerStep[] = []
  for (const step of transaction.steps) {
    const result = applyComposerStep(next, step)
    if (!result.ok) return result
    next = result.state
    inverseSteps.unshift(result.inverse)
  }

  if (transaction.selection !== undefined) {
    next = { ...next, selection: transaction.selection }
  }

  if (transaction.meta.addToHistory !== false && transaction.steps.length > 0) {
    const undo: ComposerTransaction = {
      steps: inverseSteps,
      selection: state.selection,
      meta: { source: "undo", time: transaction.meta.time, addToHistory: false },
    }
    const redo: ComposerTransaction = {
      ...transaction,
      meta: { ...transaction.meta, addToHistory: false },
    }
    next = {
      ...next,
      history: {
        done: [...next.history.done, { undo, redo }],
        undone: [],
      },
    }
  }

  return { ok: true, state: next, inverseSteps }
}

export const undoComposerState = (
  state: ComposerState,
): ComposerApplyTransactionResult => {
  const entry = state.history.done.at(-1)
  if (entry === undefined) return { ok: true, state, inverseSteps: [] }
  const withoutEntry = {
    ...state,
    history: {
      done: state.history.done.slice(0, -1),
      undone: state.history.undone,
    },
  }
  const result = applyComposerTransaction(withoutEntry, entry.undo)
  if (!result.ok) return result
  return {
    ...result,
    state: {
      ...result.state,
      history: {
        done: withoutEntry.history.done,
        undone: [...withoutEntry.history.undone, entry],
      },
    },
  }
}

export const redoComposerState = (
  state: ComposerState,
): ComposerApplyTransactionResult => {
  const entry = state.history.undone.at(-1)
  if (entry === undefined) return { ok: true, state, inverseSteps: [] }
  const withoutEntry = {
    ...state,
    history: {
      done: state.history.done,
      undone: state.history.undone.slice(0, -1),
    },
  }
  const result = applyComposerTransaction(withoutEntry, entry.redo)
  if (!result.ok) return result
  return {
    ...result,
    state: {
      ...result.state,
      history: {
        done: [...withoutEntry.history.done, entry],
        undone: withoutEntry.history.undone,
      },
    },
  }
}

export type ComposerCommand = (
  state: ComposerState,
  dispatch?: (transaction: ComposerTransaction) => void,
) => boolean

export const composerCommand = (
  transactionFor: (state: ComposerState) => ComposerTransaction | null,
): ComposerCommand =>
  (state, dispatch) => {
    const transaction = transactionFor(state)
    if (transaction === null) return false
    if (dispatch !== undefined) dispatch(transaction)
    return true
  }

export const composerCommandIds = [
  "submit",
  "stop",
  "insert_newline",
  "undo",
  "redo",
  "cancel",
  "attach",
  "toggle_preview",
  "expand",
] as const

export type ComposerCommandId = (typeof composerCommandIds)[number]

export type ComposerKeyBinding = Readonly<{
  command: ComposerCommandId
  key: string
  primary?: boolean
  shift?: boolean
  alt?: boolean
}>

export type ComposerKeyEventLike = Readonly<{
  key: string
  metaKey?: boolean
  ctrlKey?: boolean
  shiftKey?: boolean
  altKey?: boolean
}>

export const defaultComposerKeymap: ReadonlyArray<ComposerKeyBinding> = [
  { command: "submit", key: "Enter", primary: true },
  { command: "insert_newline", key: "Enter", shift: true },
  { command: "undo", key: "z", primary: true },
  { command: "redo", key: "z", primary: true, shift: true },
  { command: "cancel", key: "Escape" },
  { command: "attach", key: "u", primary: true },
  { command: "toggle_preview", key: "p", primary: true, shift: true },
  { command: "expand", key: "k", primary: true },
]

const primaryPressed = (event: ComposerKeyEventLike): boolean =>
  event.metaKey === true || event.ctrlKey === true

const bindingMatches = (
  binding: ComposerKeyBinding,
  event: ComposerKeyEventLike,
): boolean =>
  binding.key.toLowerCase() === event.key.toLowerCase() &&
  (binding.primary === true) === primaryPressed(event) &&
  (binding.shift === true) === (event.shiftKey === true) &&
  (binding.alt === true) === (event.altKey === true)

export const resolveComposerKeyBinding = (
  event: ComposerKeyEventLike,
  keymap: ReadonlyArray<ComposerKeyBinding> = defaultComposerKeymap,
): ComposerCommandId | null => {
  const match = keymap.find((binding) => bindingMatches(binding, event))
  return match?.command ?? null
}

export type ComposerInputRule = Readonly<{
  id: string
  description: string
  apply: (state: ComposerState, insertedText: string) => ComposerTransaction | null
}>

const activeTextBlock = (
  state: ComposerState,
): ComposerParagraphBlock | ComposerCodeBlock | ComposerQuoteBlock | null => {
  const index = blockIndexById(state.doc.blocks, state.selection.head.blockId)
  const block = state.doc.blocks[index]
  return block !== undefined && isTextBlock(block) ? block : null
}

const textBeforeCursor = (
  block: ComposerParagraphBlock | ComposerCodeBlock | ComposerQuoteBlock,
  selection: ComposerSelection,
): string => block.text.slice(0, clampOffset(selection.head.offset, block.text))

const setBlockKindTransaction = (
  state: ComposerState,
  kind: ComposerTextBlockKind,
  source: ComposerTransactionSource,
  language?: string,
): ComposerTransaction => ({
  steps: [
    {
      _tag: "SetBlockKind",
      blockId: state.selection.head.blockId,
      kind,
      ...(language === undefined ? {} : { language }),
    },
  ],
  meta: { source, time: Date.now() },
})

export const defaultComposerInputRules: ReadonlyArray<ComposerInputRule> = [
  {
    id: "markdown.quote",
    description: "`> ` at the start of a block turns the block into a quote.",
    apply: (state) => {
      const block = activeTextBlock(state)
      if (block === null) return null
      return textBeforeCursor(block, state.selection) === "> "
        ? setBlockKindTransaction(state, "quote", "input")
        : null
    },
  },
  {
    id: "markdown.code_fence",
    description: "Triple backticks at the start of a block turn it into code.",
    apply: (state) => {
      const block = activeTextBlock(state)
      if (block === null) return null
      const before = textBeforeCursor(block, state.selection)
      if (!before.startsWith("```")) return null
      const language = before.slice(3).trim()
      return setBlockKindTransaction(
        state,
        "code",
        "input",
        language.length > 0 ? language : undefined,
      )
    },
  },
]

export const runComposerInputRules = (
  state: ComposerState,
  insertedText: string,
  rules: ReadonlyArray<ComposerInputRule> = defaultComposerInputRules,
): ComposerTransaction | null => {
  for (const rule of rules) {
    const transaction = rule.apply(state, insertedText)
    if (transaction !== null) return transaction
  }
  return null
}

const blockIdForIndex = (index: number): ComposerBlockId =>
  composerBlockId(`block-${index + 1}`)

export const parseComposerMarkdown = (markdown: string): ComposerDoc => {
  const lines = markdown.replace(/\r\n/g, "\n").split("\n")
  const blocks: ComposerBlock[] = []
  let lineIndex = 0

  const addParagraph = (text: string): void => {
    blocks.push({
      id: blockIdForIndex(blocks.length),
      kind: "paragraph",
      text,
      marks: [],
    })
  }

  while (lineIndex < lines.length) {
    const line = lines[lineIndex] ?? ""
    if (line.trim() === "") {
      lineIndex += 1
      continue
    }

    const fence = /^```(.*)$/.exec(line)
    if (fence !== null) {
      const codeLines: string[] = []
      lineIndex += 1
      while (lineIndex < lines.length && !/^```\s*$/.test(lines[lineIndex] ?? "")) {
        codeLines.push(lines[lineIndex] ?? "")
        lineIndex += 1
      }
      if (lineIndex < lines.length) lineIndex += 1
      const language = (fence[1] ?? "").trim()
      blocks.push({
        id: blockIdForIndex(blocks.length),
        kind: "code",
        text: codeLines.join("\n"),
        ...(language.length === 0 ? {} : { language }),
      })
      continue
    }

    if (/^>\s?/.test(line)) {
      const quoteLines: string[] = []
      while (lineIndex < lines.length && /^>\s?/.test(lines[lineIndex] ?? "")) {
        quoteLines.push((lines[lineIndex] ?? "").replace(/^>\s?/, ""))
        lineIndex += 1
      }
      blocks.push({
        id: blockIdForIndex(blocks.length),
        kind: "quote",
        text: quoteLines.join("\n"),
      })
      continue
    }

    const unordered = /^[-*+]\s+(.*)$/.exec(line)
    const ordered = /^\d+[.)]\s+(.*)$/.exec(line)
    if (unordered !== null || ordered !== null) {
      const orderedList = ordered !== null
      const items: string[] = []
      while (lineIndex < lines.length) {
        const current = lines[lineIndex] ?? ""
        const match = orderedList
          ? /^\d+[.)]\s+(.*)$/.exec(current)
          : /^[-*+]\s+(.*)$/.exec(current)
        if (match === null) break
        items.push(match[1] ?? "")
        lineIndex += 1
      }
      blocks.push({
        id: blockIdForIndex(blocks.length),
        kind: "list",
        ordered: orderedList,
        items,
      })
      continue
    }

    const paragraphLines: string[] = []
    while (lineIndex < lines.length) {
      const current = lines[lineIndex] ?? ""
      if (
        current.trim() === "" ||
        /^```/.test(current) ||
        /^>\s?/.test(current) ||
        /^[-*+]\s+/.test(current) ||
        /^\d+[.)]\s+/.test(current)
      ) {
        break
      }
      paragraphLines.push(current)
      lineIndex += 1
    }
    addParagraph(paragraphLines.join("\n"))
  }

  return {
    schemaVersion: COMPOSER_SCHEMA_VERSION,
    blocks: blocks.length === 0 ? [createParagraphBlock("block-1")] : blocks,
    attachments: [],
  }
}

export const serializeComposerMarkdown = (doc: ComposerDoc): string =>
  doc.blocks
    .map((block) => {
      switch (block.kind) {
        case "paragraph":
          return block.text
        case "code":
          return `\`\`\`${block.language ?? ""}\n${block.text}\n\`\`\``
        case "quote":
          return block.text
            .split("\n")
            .map((line) => `> ${line}`)
            .join("\n")
        case "list":
          return block.items
            .map((item, index) => (block.ordered ? `${index + 1}. ${item}` : `- ${item}`))
            .join("\n")
        case "attachmentRef": {
          const attachment = doc.attachments.find(
            (candidate) => candidate.id === block.attachmentId,
          )
          const name = attachment?.name ?? block.attachmentId
          return `[attachment:${name}]`
        }
      }
    })
    .join("\n\n")
