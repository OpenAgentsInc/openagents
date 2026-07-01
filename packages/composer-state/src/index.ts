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

export const ComposerDecorationKind = S.Literals([
  "searchMatch",
  "selectionMirror",
  "remoteCursor",
  "syntaxHint",
])
export type ComposerDecorationKind = typeof ComposerDecorationKind.Type

export const ComposerDecoration = S.Struct({
  kind: ComposerDecorationKind,
  range: S.optional(
    S.Struct({
      from: S.Number,
      to: S.Number,
    }),
  ),
  ref: S.optional(S.String),
  label: S.optional(S.String),
})
export type ComposerDecoration = typeof ComposerDecoration.Type

export const ComposerListItem = S.Struct({
  text: S.String,
  children: S.Array(S.String),
})
export type ComposerListItem = typeof ComposerListItem.Type

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

export const ComposerAttachmentSource = S.Literals([
  "paste",
  "drop",
  "manual",
])
export type ComposerAttachmentSource = typeof ComposerAttachmentSource.Type

export const ComposerAttachmentSurface = S.Literals([
  "desktop-local",
  "web-hosted",
])
export type ComposerAttachmentSurface = typeof ComposerAttachmentSurface.Type

export const ComposerAttachmentUploadReceiptKind = S.Literal(
  "composer_attachment_privacy_receipt",
)
export type ComposerAttachmentUploadReceiptKind =
  typeof ComposerAttachmentUploadReceiptKind.Type

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
  contentRef: S.optional(S.String),
  thumbnailRef: S.optional(S.String),
  source: S.optional(ComposerAttachmentSource),
  status: ComposerAttachmentStatus,
  errorText: S.optional(S.String),
})
export type ComposerAttachment = typeof ComposerAttachment.Type

export const ComposerAttachmentPatch = S.Struct({
  status: S.optional(ComposerAttachmentStatus),
  digest: S.optional(S.NullOr(S.String)),
  previewUrl: S.optional(S.NullOr(S.String)),
  dimensions: S.optional(S.NullOr(ComposerAttachmentDimensions)),
  contentRef: S.optional(S.NullOr(S.String)),
  thumbnailRef: S.optional(S.NullOr(S.String)),
  source: S.optional(S.NullOr(ComposerAttachmentSource)),
  errorText: S.optional(S.NullOr(S.String)),
})
export type ComposerAttachmentPatch = typeof ComposerAttachmentPatch.Type

export const ComposerAttachmentUploadPolicy = S.Struct({
  surface: ComposerAttachmentSurface,
  maxSizeBytes: S.Number,
  allowedMimeTypes: S.Array(S.String),
  allowedMimePrefixes: S.Array(S.String),
})
export type ComposerAttachmentUploadPolicy =
  typeof ComposerAttachmentUploadPolicy.Type

export const ComposerAttachmentUploadTaskKind = S.Literals([
  "register_local_attachment",
  "upload_hosted_attachment",
  "store_thumbnail",
  "scan_attachment",
  "parse_text_attachment",
])
export type ComposerAttachmentUploadTaskKind =
  typeof ComposerAttachmentUploadTaskKind.Type

export const ComposerAttachmentUploadTask = S.Struct({
  kind: ComposerAttachmentUploadTaskKind,
  attachmentId: ComposerAttachmentId,
  surface: ComposerAttachmentSurface,
  contentRef: S.optional(S.String),
  thumbnailRef: S.optional(S.String),
})
export type ComposerAttachmentUploadTask =
  typeof ComposerAttachmentUploadTask.Type

export const ComposerAttachmentUploadReceipt = S.Struct({
  kind: ComposerAttachmentUploadReceiptKind,
  receiptRef: S.String,
  schemaVersion: ComposerSchemaVersion,
  attachmentId: ComposerAttachmentId,
  surface: ComposerAttachmentSurface,
  status: ComposerAttachmentStatus,
  name: S.String,
  mime: S.String,
  sizeBytes: S.Number,
  digest: S.optional(S.String),
  contentRef: S.optional(S.String),
  thumbnailRef: S.optional(S.String),
  dimensions: S.optional(ComposerAttachmentDimensions),
  source: S.optional(ComposerAttachmentSource),
  errorCode: S.optional(S.String),
  observedAt: S.Number,
})
export type ComposerAttachmentUploadReceipt =
  typeof ComposerAttachmentUploadReceipt.Type

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

export const ComposerUpdateAttachmentStep = S.TaggedStruct("UpdateAttachment", {
  attachmentId: ComposerAttachmentId,
  patch: ComposerAttachmentPatch,
})
export type ComposerUpdateAttachmentStep =
  typeof ComposerUpdateAttachmentStep.Type

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
  ComposerUpdateAttachmentStep,
  ComposerResizeComposerStep,
])
export type ComposerStep = typeof ComposerStep.Type

export const ComposerTransactionSource = S.Literals([
  "input",
  "paste",
  "drop",
  "manual",
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
  clientId: S.optional(S.String),
  sequence: S.optional(S.Number),
  baseVersion: S.optional(S.Number),
})
export type ComposerTransactionMeta = typeof ComposerTransactionMeta.Type

export const ComposerTransaction = S.Struct({
  steps: S.Array(ComposerStep),
  selection: S.optional(ComposerSelection),
  meta: ComposerTransactionMeta,
})
export type ComposerTransaction = typeof ComposerTransaction.Type

export const ComposerCollaborativeTransaction = S.Struct({
  schemaVersion: ComposerSchemaVersion,
  sessionRef: S.String,
  transactionRef: S.String,
  clientId: S.String,
  baseVersion: S.Number,
  version: S.Number,
  transaction: ComposerTransaction,
})
export type ComposerCollaborativeTransaction =
  typeof ComposerCollaborativeTransaction.Type

export const ComposerChangeSummary = S.Struct({
  insertedTextChars: S.Number,
  deletedTextChars: S.Number,
  replacedTextChars: S.Number,
  blockKindChanges: S.Number,
  attachmentsInserted: S.Array(ComposerAttachmentId),
  attachmentsRemoved: S.Array(ComposerAttachmentId),
  attachmentsUpdated: S.Array(ComposerAttachmentId),
  resizeChanges: S.Number,
})
export type ComposerChangeSummary = typeof ComposerChangeSummary.Type

export const ComposerRebasedTransaction = S.Struct({
  schemaVersion: ComposerSchemaVersion,
  fromBaseVersion: S.Number,
  toBaseVersion: S.Number,
  transaction: ComposerTransaction,
  changeSummary: ComposerChangeSummary,
})
export type ComposerRebasedTransaction = typeof ComposerRebasedTransaction.Type

export const ComposerAttachmentUploadPlan = S.Struct({
  attachmentId: ComposerAttachmentId,
  surface: ComposerAttachmentSurface,
  transaction: ComposerTransaction,
  tasks: S.Array(ComposerAttachmentUploadTask),
  receipt: ComposerAttachmentUploadReceipt,
})
export type ComposerAttachmentUploadPlan =
  typeof ComposerAttachmentUploadPlan.Type

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

export type ComposerFileLike = Readonly<{
  name?: string
  type?: string
  size?: number
  previewUrl?: string
  contentRef?: string
  thumbnailRef?: string
  dimensions?: ComposerAttachmentDimensions
}>

export type ComposerAttachmentDeferredTaskKind =
  | "create_image_thumbnail"
  | "extract_image_dimensions"
  | "count_text_bytes"
  | "estimate_text_tokens"
  | "detect_snippet_language"

export type ComposerAttachmentDeferredTask = Readonly<{
  kind: ComposerAttachmentDeferredTaskKind
  attachmentId: ComposerAttachmentId
}>

export type ComposerAttachmentUploadErrorCode =
  | "file_too_large"
  | "mime_not_allowed"
  | "missing_digest"
  | "attachment_not_found"

export type ComposerAttachmentUploadPlanResult =
  | Readonly<{
      ok: true
      plan: ComposerAttachmentUploadPlan
    }>
  | Readonly<{
      ok: false
      errorCode: ComposerAttachmentUploadErrorCode
      transaction: ComposerTransaction
      receipt: ComposerAttachmentUploadReceipt
    }>

export type ComposerStageAttachmentFilesOptions = Readonly<{
  source: ComposerAttachmentSource
  at?: ComposerBlockPosition
  idPrefix?: string
}>

export type ComposerStageAttachmentFilesResult = Readonly<{
  attachments: ReadonlyArray<ComposerAttachment>
  transaction: ComposerTransaction
  deferredTasks: ReadonlyArray<ComposerAttachmentDeferredTask>
}>

export const DEFAULT_LARGE_TEXT_ATTACHMENT_THRESHOLD = 16_000

export const DEFAULT_DESKTOP_LOCAL_ATTACHMENT_UPLOAD_POLICY: ComposerAttachmentUploadPolicy =
  {
    surface: "desktop-local",
    maxSizeBytes: 50 * 1024 * 1024,
    allowedMimeTypes: [],
    allowedMimePrefixes: [],
  }

export const DEFAULT_WEB_HOSTED_ATTACHMENT_UPLOAD_POLICY: ComposerAttachmentUploadPolicy =
  {
    surface: "web-hosted",
    maxSizeBytes: 25 * 1024 * 1024,
    allowedMimeTypes: [
      "application/json",
      "application/pdf",
      "application/zip",
      "application/octet-stream",
    ],
    allowedMimePrefixes: ["image/", "text/"],
  }

export type ComposerLargeTextPasteOffer = Readonly<{
  offered: boolean
  textBytes: number
  thresholdBytes: number
  transaction: ComposerTransaction | null
  attachment?: ComposerAttachment
  deferredTasks: ReadonlyArray<ComposerAttachmentDeferredTask>
}>

const textEncoder = new TextEncoder()

const byteLength = (text: string): number => textEncoder.encode(text).byteLength

const slugForAttachmentName = (name: string): string => {
  const slug = name
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
  return slug.length === 0 ? "attachment" : slug.slice(0, 80)
}

const defaultFileNameFor = (
  source: ComposerAttachmentSource,
  index: number,
): string => `${source}-attachment-${index + 1}`

const attachmentIdFor = (
  source: ComposerAttachmentSource,
  index: number,
  name: string,
  sizeBytes: number,
): ComposerAttachmentId =>
  composerAttachmentId(
    `${source}-${index + 1}-${slugForAttachmentName(name)}-${Math.max(0, sizeBytes)}`,
  )

const extensionForName = (name: string): string => {
  const index = name.lastIndexOf(".")
  return index < 0 ? "" : name.slice(index + 1).toLowerCase()
}

export const inferComposerAttachmentKind = (
  mime: string,
  name: string,
): ComposerAttachmentKind => {
  if (mime.startsWith("image/")) return "image"
  if (mime.startsWith("text/")) return "text"
  if (
    [
      "c",
      "cpp",
      "css",
      "diff",
      "go",
      "html",
      "js",
      "json",
      "jsx",
      "md",
      "py",
      "rs",
      "sh",
      "sql",
      "ts",
      "tsx",
      "yaml",
      "yml",
    ].includes(extensionForName(name))
  ) {
    return "snippet"
  }
  return "file"
}

export const deferredComposerAttachmentTasks = (
  attachment: ComposerAttachment,
): ReadonlyArray<ComposerAttachmentDeferredTask> => {
  if (attachment.kind === "image") {
    return [
      ...(attachment.previewUrl === undefined
        ? [{ kind: "create_image_thumbnail" as const, attachmentId: attachment.id }]
        : []),
      ...(attachment.dimensions === undefined
        ? [{ kind: "extract_image_dimensions" as const, attachmentId: attachment.id }]
        : []),
    ]
  }

  if (attachment.kind === "text" || attachment.kind === "snippet") {
    return [
      { kind: "count_text_bytes", attachmentId: attachment.id },
      { kind: "estimate_text_tokens", attachmentId: attachment.id },
      ...(attachment.kind === "snippet"
        ? [{ kind: "detect_snippet_language" as const, attachmentId: attachment.id }]
        : []),
    ]
  }

  return []
}

export const createComposerAttachment = (input: {
  id: ComposerAttachmentId
  kind: ComposerAttachmentKind
  name: string
  mime: string
  sizeBytes: number
  source?: ComposerAttachmentSource
  status?: ComposerAttachmentStatus
  digest?: string
  previewUrl?: string
  dimensions?: ComposerAttachmentDimensions
  contentRef?: string
  thumbnailRef?: string
  errorText?: string
}): ComposerAttachment => ({
  id: input.id,
  kind: input.kind,
  name: input.name,
  mime: input.mime,
  sizeBytes: Math.max(0, Math.trunc(input.sizeBytes)),
  ...(input.source === undefined ? {} : { source: input.source }),
  status: input.status ?? "staged",
  ...(input.digest === undefined ? {} : { digest: input.digest }),
  ...(input.previewUrl === undefined ? {} : { previewUrl: input.previewUrl }),
  ...(input.dimensions === undefined ? {} : { dimensions: input.dimensions }),
  ...(input.contentRef === undefined ? {} : { contentRef: input.contentRef }),
  ...(input.thumbnailRef === undefined
    ? {}
    : { thumbnailRef: input.thumbnailRef }),
  ...(input.errorText === undefined ? {} : { errorText: input.errorText }),
})

const positionForAttachmentIndex = (
  at: ComposerBlockPosition | undefined,
  offset: number,
): ComposerBlockPosition | undefined =>
  at?.index === undefined ? at : { index: at.index + offset }

export const stageComposerAttachmentFiles = (
  files: ReadonlyArray<ComposerFileLike>,
  options: ComposerStageAttachmentFilesOptions,
): ComposerStageAttachmentFilesResult => {
  const attachments = files.map((file, index): ComposerAttachment => {
    const name = file.name ?? defaultFileNameFor(options.source, index)
    const mime =
      file.type === undefined || file.type.trim() === ""
        ? "application/octet-stream"
        : file.type
    const sizeBytes = Math.max(0, Math.trunc(file.size ?? 0))
    return createComposerAttachment({
      id:
        options.idPrefix === undefined
          ? attachmentIdFor(options.source, index, name, sizeBytes)
          : composerAttachmentId(`${options.idPrefix}-${index + 1}`),
      kind: inferComposerAttachmentKind(mime, name),
      name,
      mime,
      sizeBytes,
      source: options.source,
      status: "staged",
      ...(file.previewUrl === undefined ? {} : { previewUrl: file.previewUrl }),
      ...(file.dimensions === undefined ? {} : { dimensions: file.dimensions }),
      ...(file.contentRef === undefined ? {} : { contentRef: file.contentRef }),
      ...(file.thumbnailRef === undefined
        ? {}
        : { thumbnailRef: file.thumbnailRef }),
    })
  })
  return {
    attachments,
    transaction: {
      steps: attachments.map((attachment, index) => ({
        _tag: "InsertAttachment" as const,
        attachment,
        ...(positionForAttachmentIndex(options.at, index) === undefined
          ? {}
          : { at: positionForAttachmentIndex(options.at, index) }),
      })),
      meta: { source: options.source, time: Date.now() },
    },
    deferredTasks: attachments.flatMap((attachment) =>
      deferredComposerAttachmentTasks(attachment),
    ),
  }
}

export const stageComposerPastedFiles = (
  files: ReadonlyArray<ComposerFileLike>,
  options: Omit<ComposerStageAttachmentFilesOptions, "source"> = {},
): ComposerStageAttachmentFilesResult =>
  stageComposerAttachmentFiles(files, { ...options, source: "paste" })

export const stageComposerDroppedFiles = (
  files: ReadonlyArray<ComposerFileLike>,
  options: Omit<ComposerStageAttachmentFilesOptions, "source"> = {},
): ComposerStageAttachmentFilesResult =>
  stageComposerAttachmentFiles(files, { ...options, source: "drop" })

export const offerComposerLargeTextPaste = (
  text: string,
  options: Readonly<{
    thresholdBytes?: number
    name?: string
    contentRef?: string
    at?: ComposerBlockPosition
    id?: ComposerAttachmentId
  }> = {},
): ComposerLargeTextPasteOffer => {
  const textBytes = byteLength(text)
  const thresholdBytes =
    options.thresholdBytes ?? DEFAULT_LARGE_TEXT_ATTACHMENT_THRESHOLD
  if (textBytes < thresholdBytes) {
    return {
      offered: false,
      textBytes,
      thresholdBytes,
      transaction: null,
      deferredTasks: [],
    }
  }

  const name = options.name ?? "pasted-text.txt"
  const attachment = createComposerAttachment({
    id: options.id ?? attachmentIdFor("paste", 0, name, textBytes),
    kind: "text",
    name,
    mime: "text/plain",
    sizeBytes: textBytes,
    source: "paste",
    status: "staged",
    contentRef:
      options.contentRef ??
      `local-text:${attachmentIdFor("paste", 0, name, textBytes)}`,
  })
  return {
    offered: true,
    textBytes,
    thresholdBytes,
    attachment,
    transaction: {
      steps: [
        {
          _tag: "InsertAttachment",
          attachment,
          ...(options.at === undefined ? {} : { at: options.at }),
        },
      ],
      meta: { source: "paste", time: Date.now() },
    },
    deferredTasks: deferredComposerAttachmentTasks(attachment),
  }
}

const normalizeDigestSegment = (digest: string): string => {
  const trimmed = digest.trim().toLowerCase()
  const withoutPrefix = trimmed.startsWith("sha256:")
    ? trimmed.slice("sha256:".length)
    : trimmed
  return withoutPrefix.replace(/[^a-f0-9]/g, "")
}

const publicAttachmentName = (name: string): string =>
  slugForAttachmentName(name).slice(0, 80)

export const composerAttachmentContentAddressedRef = (input: {
  surface: ComposerAttachmentSurface
  digest: string
  name?: string
}): string => {
  const digest = normalizeDigestSegment(input.digest)
  const name = input.name === undefined ? "" : `.${publicAttachmentName(input.name)}`
  return `attachment.${input.surface}.sha256.${digest}${name}`
}

export const composerAttachmentThumbnailRef = (input: {
  surface: ComposerAttachmentSurface
  digest: string
  attachmentId: ComposerAttachmentId
}): string => {
  const digest = normalizeDigestSegment(input.digest)
  return `attachment_thumbnail.${input.surface}.sha256.${digest}.${input.attachmentId}`
}

const receiptRefForAttachment = (input: {
  attachmentId: ComposerAttachmentId
  surface: ComposerAttachmentSurface
  status: ComposerAttachmentStatus
  digest?: string
  errorCode?: string
}): string => {
  const digest =
    input.digest === undefined ? "no_digest" : normalizeDigestSegment(input.digest)
  const suffix = input.errorCode ?? input.status
  return `receipt.composer_attachment.${input.surface}.${input.attachmentId}.${suffix}.${digest}`
}

export const projectComposerAttachmentUploadReceipt = (input: {
  attachment: ComposerAttachment
  surface: ComposerAttachmentSurface
  observedAt?: number
  errorCode?: string
}): ComposerAttachmentUploadReceipt => ({
  kind: "composer_attachment_privacy_receipt",
  schemaVersion: COMPOSER_SCHEMA_VERSION,
  receiptRef: receiptRefForAttachment({
    attachmentId: input.attachment.id,
    surface: input.surface,
    status: input.attachment.status,
    ...(input.attachment.digest === undefined
      ? {}
      : { digest: input.attachment.digest }),
    ...(input.errorCode === undefined ? {} : { errorCode: input.errorCode }),
  }),
  attachmentId: input.attachment.id,
  surface: input.surface,
  status: input.attachment.status,
  name: input.attachment.name,
  mime: input.attachment.mime,
  sizeBytes: input.attachment.sizeBytes,
  ...(input.attachment.digest === undefined
    ? {}
    : { digest: input.attachment.digest }),
  ...(input.attachment.contentRef === undefined
    ? {}
    : { contentRef: input.attachment.contentRef }),
  ...(input.attachment.thumbnailRef === undefined
    ? {}
    : { thumbnailRef: input.attachment.thumbnailRef }),
  ...(input.attachment.dimensions === undefined
    ? {}
    : { dimensions: input.attachment.dimensions }),
  ...(input.attachment.source === undefined
    ? {}
    : { source: input.attachment.source }),
  ...(input.errorCode === undefined ? {} : { errorCode: input.errorCode }),
  observedAt: input.observedAt ?? Date.now(),
})

const mimeAllowedByPolicy = (
  mime: string,
  policy: ComposerAttachmentUploadPolicy,
): boolean =>
  policy.allowedMimeTypes.length === 0 && policy.allowedMimePrefixes.length === 0
    ? true
    : policy.allowedMimeTypes.includes(mime) ||
      policy.allowedMimePrefixes.some((prefix) => mime.startsWith(prefix))

const attachmentUploadFailure = (input: {
  attachment: ComposerAttachment
  policy: ComposerAttachmentUploadPolicy
  errorCode: ComposerAttachmentUploadErrorCode
  errorText: string
  time: number
}): ComposerAttachmentUploadPlanResult => {
  const transaction: ComposerTransaction = {
    steps: [
      {
        _tag: "UpdateAttachment",
        attachmentId: input.attachment.id,
        patch: { status: "error", errorText: input.errorText },
      },
    ],
    meta: { source: "program", time: input.time },
  }
  return {
    ok: false,
    errorCode: input.errorCode,
    transaction,
    receipt: projectComposerAttachmentUploadReceipt({
      attachment: {
        ...input.attachment,
        status: "error",
        errorText: input.errorText,
      },
      surface: input.policy.surface,
      observedAt: input.time,
      errorCode: input.errorCode,
    }),
  }
}

export const planComposerAttachmentUpload = (
  state: ComposerState,
  attachmentId: ComposerAttachmentId,
  policy: ComposerAttachmentUploadPolicy,
  time = Date.now(),
): ComposerAttachmentUploadPlanResult => {
  const attachment = state.doc.attachments.find(
    (candidate) => candidate.id === attachmentId,
  )
  if (attachment === undefined) {
    const transaction: ComposerTransaction = {
      steps: [],
      meta: { source: "program", time, addToHistory: false },
    }
    return {
      ok: false,
      errorCode: "attachment_not_found",
      transaction,
      receipt: {
        kind: "composer_attachment_privacy_receipt",
        schemaVersion: COMPOSER_SCHEMA_VERSION,
        receiptRef: receiptRefForAttachment({
          attachmentId,
          surface: policy.surface,
          status: "error",
          errorCode: "attachment_not_found",
        }),
        attachmentId,
        surface: policy.surface,
        status: "error",
        name: "",
        mime: "",
        sizeBytes: 0,
        errorCode: "attachment_not_found",
        observedAt: time,
      },
    }
  }

  if (attachment.sizeBytes > policy.maxSizeBytes) {
    return attachmentUploadFailure({
      attachment,
      policy,
      errorCode: "file_too_large",
      errorText: "Attachment is too large.",
      time,
    })
  }

  if (!mimeAllowedByPolicy(attachment.mime, policy)) {
    return attachmentUploadFailure({
      attachment,
      policy,
      errorCode: "mime_not_allowed",
      errorText: "Attachment type is not supported.",
      time,
    })
  }

  const transaction: ComposerTransaction = {
    steps: [
      {
        _tag: "UpdateAttachment",
        attachmentId,
        patch: { status: "uploading", errorText: null },
      },
    ],
    meta: { source: "program", time },
  }
  const uploadTaskKind =
    policy.surface === "desktop-local"
      ? "register_local_attachment"
      : "upload_hosted_attachment"
  const tasks: ComposerAttachmentUploadTask[] = [
    {
      kind: uploadTaskKind,
      attachmentId,
      surface: policy.surface,
      ...(attachment.contentRef === undefined
        ? {}
        : { contentRef: attachment.contentRef }),
    },
    { kind: "scan_attachment", attachmentId, surface: policy.surface },
    ...(attachment.kind === "text" || attachment.kind === "snippet"
      ? [{ kind: "parse_text_attachment" as const, attachmentId, surface: policy.surface }]
      : []),
    ...(attachment.kind === "image" && attachment.thumbnailRef === undefined
      ? [{ kind: "store_thumbnail" as const, attachmentId, surface: policy.surface }]
      : []),
  ]

  return {
    ok: true,
    plan: {
      attachmentId,
      surface: policy.surface,
      transaction,
      tasks,
      receipt: projectComposerAttachmentUploadReceipt({
        attachment: { ...attachment, status: "uploading" },
        surface: policy.surface,
        observedAt: time,
      }),
    },
  }
}

export const readyComposerAttachmentTransaction = (
  state: ComposerState,
  attachmentId: ComposerAttachmentId,
  input: Readonly<{
    surface: ComposerAttachmentSurface
    digest: string
    thumbnailDigest?: string
    dimensions?: ComposerAttachmentDimensions
    time?: number
  }>,
): ComposerTransaction | null => {
  const attachment = state.doc.attachments.find(
    (candidate) => candidate.id === attachmentId,
  )
  if (attachment === undefined) return null
  const contentRef = composerAttachmentContentAddressedRef({
    surface: input.surface,
    digest: input.digest,
    name: attachment.name,
  })
  return {
    steps: [
      {
        _tag: "UpdateAttachment",
        attachmentId,
        patch: {
          status: "ready",
          digest: input.digest,
          contentRef,
          errorText: null,
          ...(input.thumbnailDigest === undefined
            ? {}
            : {
                thumbnailRef: composerAttachmentThumbnailRef({
                  surface: input.surface,
                  digest: input.thumbnailDigest,
                  attachmentId,
                }),
              }),
          ...(input.dimensions === undefined
            ? {}
            : { dimensions: input.dimensions }),
        },
      },
    ],
    meta: { source: "program", time: input.time ?? Date.now() },
  }
}

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

export const mapComposerStepThroughStep = (
  step: ComposerStep,
  remoteStep: ComposerStep,
): ComposerStep => {
  switch (step._tag) {
    case "InsertText":
      return {
        ...step,
        at: mapPositionThroughStep(step.at, remoteStep),
      }
    case "DeleteRange":
      return {
        ...step,
        range: {
          anchor: mapPositionThroughStep(step.range.anchor, remoteStep),
          head: mapPositionThroughStep(step.range.head, remoteStep),
        },
      }
    case "ReplaceRange":
      return {
        ...step,
        range: {
          anchor: mapPositionThroughStep(step.range.anchor, remoteStep),
          head: mapPositionThroughStep(step.range.head, remoteStep),
        },
      }
    case "SetBlockKind":
    case "InsertAttachment":
    case "RemoveAttachment":
    case "UpdateAttachment":
    case "ResizeComposer":
      return step
  }
}

export const mapComposerTransactionThroughSteps = (
  transaction: ComposerTransaction,
  remoteSteps: ReadonlyArray<ComposerStep>,
): ComposerTransaction => {
  const steps = transaction.steps.map((step) =>
    remoteSteps.reduce(
      (mappedStep, remoteStep) => mapComposerStepThroughStep(mappedStep, remoteStep),
      step,
    ),
  )
  const selection =
    transaction.selection === undefined
      ? undefined
      : remoteSteps.reduce(
          (mappedSelection, remoteStep) =>
            mapSelectionThroughStep(mappedSelection, remoteStep),
          transaction.selection,
        )
  return {
    ...transaction,
    steps,
    ...(selection === undefined ? {} : { selection }),
  }
}

const rangeCharLength = (range: ComposerTextRange): number =>
  Math.abs(Math.trunc(range.head.offset) - Math.trunc(range.anchor.offset))

const uniqueComposerAttachmentIds = (
  ids: ReadonlyArray<ComposerAttachmentId>,
): ReadonlyArray<ComposerAttachmentId> => [...new Set(ids)]

export const summarizeComposerSteps = (
  steps: ReadonlyArray<ComposerStep>,
): ComposerChangeSummary => {
  let insertedTextChars = 0
  let deletedTextChars = 0
  let replacedTextChars = 0
  let blockKindChanges = 0
  let resizeChanges = 0
  const attachmentsInserted: ComposerAttachmentId[] = []
  const attachmentsRemoved: ComposerAttachmentId[] = []
  const attachmentsUpdated: ComposerAttachmentId[] = []

  for (const step of steps) {
    switch (step._tag) {
      case "InsertText":
        insertedTextChars += step.text.length
        break
      case "DeleteRange":
        deletedTextChars += rangeCharLength(step.range)
        break
      case "ReplaceRange":
        replacedTextChars += rangeCharLength(step.range)
        insertedTextChars += step.text.length
        break
      case "SetBlockKind":
        blockKindChanges += 1
        break
      case "InsertAttachment":
        attachmentsInserted.push(step.attachment.id)
        break
      case "RemoveAttachment":
        attachmentsRemoved.push(step.attachmentId)
        break
      case "UpdateAttachment":
        attachmentsUpdated.push(step.attachmentId)
        break
      case "ResizeComposer":
        resizeChanges += 1
        break
    }
  }

  return {
    insertedTextChars,
    deletedTextChars,
    replacedTextChars,
    blockKindChanges,
    attachmentsInserted: uniqueComposerAttachmentIds(attachmentsInserted),
    attachmentsRemoved: uniqueComposerAttachmentIds(attachmentsRemoved),
    attachmentsUpdated: uniqueComposerAttachmentIds(attachmentsUpdated),
    resizeChanges,
  }
}

export const createComposerCollaborativeTransaction = (input: {
  sessionRef: string
  transactionRef: string
  clientId: string
  baseVersion: number
  transaction: ComposerTransaction
}): ComposerCollaborativeTransaction => ({
  schemaVersion: COMPOSER_SCHEMA_VERSION,
  sessionRef: input.sessionRef,
  transactionRef: input.transactionRef,
  clientId: input.clientId,
  baseVersion: Math.max(0, Math.trunc(input.baseVersion)),
  version: Math.max(0, Math.trunc(input.baseVersion)) + input.transaction.steps.length,
  transaction: {
    ...input.transaction,
    meta: {
      ...input.transaction.meta,
      clientId: input.clientId,
      baseVersion: Math.max(0, Math.trunc(input.baseVersion)),
    },
  },
})

export const rebaseComposerTransaction = (input: {
  transaction: ComposerTransaction
  fromBaseVersion: number
  remoteSteps: ReadonlyArray<ComposerStep>
}): ComposerRebasedTransaction => {
  const fromBaseVersion = Math.max(0, Math.trunc(input.fromBaseVersion))
  const transaction = mapComposerTransactionThroughSteps(
    input.transaction,
    input.remoteSteps,
  )
  return {
    schemaVersion: COMPOSER_SCHEMA_VERSION,
    fromBaseVersion,
    toBaseVersion: fromBaseVersion + input.remoteSteps.length,
    transaction: {
      ...transaction,
      meta: {
        ...transaction.meta,
        baseVersion: fromBaseVersion + input.remoteSteps.length,
      },
    },
    changeSummary: summarizeComposerSteps(transaction.steps),
  }
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

type MutableAttachment = {
  -readonly [K in keyof ComposerAttachment]: ComposerAttachment[K]
}

const patchHas = (
  patch: ComposerAttachmentPatch,
  key: keyof ComposerAttachmentPatch,
): boolean => Object.prototype.hasOwnProperty.call(patch, key)

const applyAttachmentPatch = (
  attachment: ComposerAttachment,
  patch: ComposerAttachmentPatch,
): ComposerAttachment => {
  const next: MutableAttachment = { ...attachment }
  if (patch.status !== undefined) next.status = patch.status
  if (patchHas(patch, "digest")) {
    if (patch.digest === null) delete next.digest
    else if (patch.digest !== undefined) next.digest = patch.digest
  }
  if (patchHas(patch, "previewUrl")) {
    if (patch.previewUrl === null) delete next.previewUrl
    else if (patch.previewUrl !== undefined) next.previewUrl = patch.previewUrl
  }
  if (patchHas(patch, "dimensions")) {
    if (patch.dimensions === null) delete next.dimensions
    else if (patch.dimensions !== undefined) next.dimensions = patch.dimensions
  }
  if (patchHas(patch, "contentRef")) {
    if (patch.contentRef === null) delete next.contentRef
    else if (patch.contentRef !== undefined) next.contentRef = patch.contentRef
  }
  if (patchHas(patch, "thumbnailRef")) {
    if (patch.thumbnailRef === null) delete next.thumbnailRef
    else if (patch.thumbnailRef !== undefined) next.thumbnailRef = patch.thumbnailRef
  }
  if (patchHas(patch, "source")) {
    if (patch.source === null) delete next.source
    else if (patch.source !== undefined) next.source = patch.source
  }
  if (patchHas(patch, "errorText")) {
    if (patch.errorText === null) delete next.errorText
    else if (patch.errorText !== undefined) next.errorText = patch.errorText
  }
  return next
}

const inverseAttachmentPatch = (
  attachment: ComposerAttachment,
  patch: ComposerAttachmentPatch,
): ComposerAttachmentPatch => ({
  ...(patch.status === undefined ? {} : { status: attachment.status }),
  ...(patchHas(patch, "digest")
    ? { digest: attachment.digest ?? null }
    : {}),
  ...(patchHas(patch, "previewUrl")
    ? { previewUrl: attachment.previewUrl ?? null }
    : {}),
  ...(patchHas(patch, "dimensions")
    ? { dimensions: attachment.dimensions ?? null }
    : {}),
  ...(patchHas(patch, "contentRef")
    ? { contentRef: attachment.contentRef ?? null }
    : {}),
  ...(patchHas(patch, "thumbnailRef")
    ? { thumbnailRef: attachment.thumbnailRef ?? null }
    : {}),
  ...(patchHas(patch, "source") ? { source: attachment.source ?? null } : {}),
  ...(patchHas(patch, "errorText")
    ? { errorText: attachment.errorText ?? null }
    : {}),
})

const applyUpdateAttachment = (
  state: ComposerState,
  step: ComposerUpdateAttachmentStep,
): ComposerApplyStepResult => {
  const index = state.doc.attachments.findIndex(
    (attachment) => attachment.id === step.attachmentId,
  )
  const attachment = state.doc.attachments[index]
  if (attachment === undefined) {
    return err("attachment_not_found", `Attachment ${step.attachmentId} was not found`)
  }
  const nextAttachment = applyAttachmentPatch(attachment, step.patch)
  const attachments = [
    ...state.doc.attachments.slice(0, index),
    nextAttachment,
    ...state.doc.attachments.slice(index + 1),
  ]
  return {
    ok: true,
    state: { ...state, doc: { ...state.doc, attachments } },
    inverse: {
      _tag: "UpdateAttachment",
      attachmentId: step.attachmentId,
      patch: inverseAttachmentPatch(attachment, step.patch),
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
    case "UpdateAttachment":
      return applyUpdateAttachment(state, step)
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

export const retryComposerAttachmentTransaction = (
  state: ComposerState,
  attachmentId: ComposerAttachmentId,
  time = Date.now(),
): ComposerTransaction | null => {
  const attachment = state.doc.attachments.find(
    (candidate) => candidate.id === attachmentId,
  )
  if (attachment === undefined) return null
  return {
    steps: [
      {
        _tag: "UpdateAttachment",
        attachmentId,
        patch: { status: "staged", errorText: null },
      },
    ],
    meta: { source: "program", time },
  }
}

export const setComposerAttachmentStatusTransaction = (
  state: ComposerState,
  attachmentId: ComposerAttachmentId,
  patch: ComposerAttachmentPatch,
  time = Date.now(),
): ComposerTransaction | null => {
  if (!state.doc.attachments.some((attachment) => attachment.id === attachmentId)) {
    return null
  }
  return {
    steps: [{ _tag: "UpdateAttachment", attachmentId, patch }],
    meta: { source: "program", time },
  }
}

export type ComposerAttachmentNavigationDirection =
  | "first"
  | "last"
  | "next"
  | "previous"
  | "clear"

const orderedAttachmentIds = (
  state: ComposerState,
): ReadonlyArray<ComposerAttachmentId> => {
  const idsFromBlocks = state.doc.blocks.flatMap((block) =>
    block.kind === "attachmentRef" ? [block.attachmentId] : [],
  )
  const blockIds = new Set(idsFromBlocks)
  return [
    ...idsFromBlocks,
    ...state.doc.attachments
      .map((attachment) => attachment.id)
      .filter((id) => !blockIds.has(id)),
  ]
}

export const selectComposerAttachment = (
  state: ComposerState,
  attachmentId: ComposerAttachmentId | undefined,
): ComposerState => {
  if (attachmentId === undefined) {
    const { selectedAttachmentId: _selectedAttachmentId, ...selection } =
      state.selection
    return { ...state, selection }
  }
  return {
    ...state,
    selection: { ...state.selection, selectedAttachmentId: attachmentId },
  }
}

export const moveComposerAttachmentSelection = (
  state: ComposerState,
  direction: ComposerAttachmentNavigationDirection,
): ComposerState => {
  if (direction === "clear") return selectComposerAttachment(state, undefined)
  const ids = orderedAttachmentIds(state)
  if (ids.length === 0) return selectComposerAttachment(state, undefined)
  if (direction === "first") return selectComposerAttachment(state, ids[0])
  if (direction === "last") return selectComposerAttachment(state, ids.at(-1))
  const selected = state.selection.selectedAttachmentId
  const index = selected === undefined ? -1 : ids.indexOf(selected)
  if (direction === "next") {
    return selectComposerAttachment(
      state,
      ids[Math.min(ids.length - 1, index + 1)] ?? ids[0],
    )
  }
  return selectComposerAttachment(
    state,
    ids[index <= 0 ? ids.length - 1 : index - 1],
  )
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
  "select_next_attachment",
  "select_previous_attachment",
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
  { command: "select_previous_attachment", key: "ArrowLeft", alt: true },
  { command: "select_next_attachment", key: "ArrowRight", alt: true },
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
