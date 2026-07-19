import { Schema } from "effect"

export const IdeDocumentRef = Schema.String.check(
  Schema.isPattern(/^ide\.document\.[a-z0-9]{12}\.[0-9]+$/),
).pipe(Schema.brand("IdeDocumentRef"))
export type IdeDocumentRef = typeof IdeDocumentRef.Type

export const IdeDocumentGeneration = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
).pipe(Schema.brand("IdeDocumentGeneration"))
export type IdeDocumentGeneration = typeof IdeDocumentGeneration.Type

export const IdeDocumentSequence = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
).pipe(Schema.brand("IdeDocumentSequence"))
export type IdeDocumentSequence = typeof IdeDocumentSequence.Type

export const IdeMonacoModelVersion = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(1),
).pipe(Schema.brand("IdeMonacoModelVersion"))
export type IdeMonacoModelVersion = typeof IdeMonacoModelVersion.Type

export const IdeEditorViewRef = Schema.String.check(
  Schema.isPattern(/^ide\.view\.[a-z0-9._-]{1,80}$/),
).pipe(Schema.brand("IdeEditorViewRef"))
export type IdeEditorViewRef = typeof IdeEditorViewRef.Type

export const IdeEditorSelectionSchema = Schema.Struct({
  start: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  end: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
})
export type IdeEditorSelection = typeof IdeEditorSelectionSchema.Type

export const IdeMonacoTextChangeSchema = Schema.Struct({
  offset: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  length: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  text: Schema.String.check(Schema.isMaxLength(1_000_000)),
})
export type IdeMonacoTextChange = typeof IdeMonacoTextChangeSchema.Type

export const IdeMonacoDocumentEventSchema = Schema.TaggedUnion({
  Edit: {
    documentRef: IdeDocumentRef,
    generation: IdeDocumentGeneration,
    sequence: IdeDocumentSequence,
    modelVersion: IdeMonacoModelVersion,
    value: Schema.String.check(Schema.isMaxLength(1_000_000)),
    changes: Schema.Array(IdeMonacoTextChangeSchema).check(Schema.isMaxLength(1_000)),
  },
  Selection: {
    documentRef: IdeDocumentRef,
    generation: IdeDocumentGeneration,
    viewRef: IdeEditorViewRef,
    selection: IdeEditorSelectionSchema,
  },
  Save: {
    documentRef: IdeDocumentRef,
    generation: IdeDocumentGeneration,
  },
  Close: {
    documentRef: IdeDocumentRef,
    generation: IdeDocumentGeneration,
    force: Schema.Boolean,
  },
})
export type IdeMonacoDocumentEvent = typeof IdeMonacoDocumentEventSchema.Type

export const IdeVimModeSchema = Schema.Literals([
  "normal",
  "insert",
  "visual",
  "visual_line",
  "visual_block",
  "replace",
  "operator_pending",
])
export type IdeVimMode = typeof IdeVimModeSchema.Type

export const IdeVimProjectionSchema = Schema.Struct({
  enabled: Schema.Boolean,
  mode: IdeVimModeSchema,
  pending: Schema.NullOr(Schema.String.check(Schema.isMaxLength(80))),
  count: Schema.NullOr(Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 1, maximum: 9_999 }))),
})
export type IdeVimProjection = typeof IdeVimProjectionSchema.Type

export const IdeMonacoRuntimeResourceSnapshotSchema = Schema.Struct({
  state: Schema.Literals(["idle", "loading", "ready", "failed", "stopped"]),
  modelCount: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  viewCount: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  workerCount: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  listenerCount: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  vimHandlerCount: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
})
export type IdeMonacoRuntimeResourceSnapshot = typeof IdeMonacoRuntimeResourceSnapshotSchema.Type

const fnv1a = (value: string): string => {
  let hash = 0x811c9dc5
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index)
    hash = Math.imul(hash, 0x01000193)
  }
  const first = (hash >>> 0).toString(36).padStart(7, "0")
  let secondHash = 0x9e3779b9
  for (let index = value.length - 1; index >= 0; index -= 1) {
    secondHash ^= value.charCodeAt(index)
    secondHash = Math.imul(secondHash, 0x85ebca6b)
  }
  return `${first}${(secondHash >>> 0).toString(36).padStart(7, "0")}`.slice(0, 12)
}

/**
 * Derives an opaque, non-authoritative model identity from the admitted grant
 * generation plus an editor-owned ordinal. Paths never enter model identity,
 * so a confirmed rename/move cannot replace the document or its draft.
 */
export const makeIdeDocumentRef = (grantGenerationRef: string, ordinal: number): IdeDocumentRef =>
  IdeDocumentRef.make(`ide.document.${fnv1a(grantGenerationRef)}.${Math.max(0, Math.trunc(ordinal))}`)

export const decodeIdeMonacoDocumentEvent = Schema.decodeUnknownEffect(IdeMonacoDocumentEventSchema)
