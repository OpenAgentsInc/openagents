import { Effect, Schema } from "effect"

import {
  IdeDocumentGeneration,
  IdeDocumentRef,
  IdeDocumentSequence,
  IdeEditorSelectionSchema,
  IdeEditorViewRef,
  IdeMonacoDocumentEventSchema,
  IdeMonacoRuntimeResourceSnapshotSchema,
  IdeVimProjectionSchema,
  type IdeEditorSelection,
  type IdeMonacoDocumentEvent,
  type IdeMonacoRuntimeResourceSnapshot,
  type IdeVimProjection,
} from "./monaco-document-contract.ts"
import { IdeMonacoEditorOptionsSchema } from "./workbench-contract.ts"

export const IdeMonacoAttachInputSchema = Schema.Struct({
  documentRef: IdeDocumentRef,
  generation: IdeDocumentGeneration,
  sequence: IdeDocumentSequence,
  viewRef: IdeEditorViewRef,
  pathLabel: Schema.String.check(Schema.isMaxLength(320)),
  language: Schema.String.check(Schema.isMaxLength(80)),
  value: Schema.String.check(Schema.isMaxLength(1_000_000)),
  selection: IdeEditorSelectionSchema,
  selectionVersion: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  wordWrap: Schema.Boolean,
  minimap: Schema.Boolean,
  vimEnabled: Schema.Boolean,
  editorOptions: IdeMonacoEditorOptionsSchema,
  readOnly: Schema.Boolean,
})
export type IdeMonacoAttachInput = typeof IdeMonacoAttachInputSchema.Type

export interface IdeMonacoMountedView {
  readonly update: (input: IdeMonacoAttachInput) => void
  readonly focus: () => void
  readonly dispose: () => void
}

export interface IdeMonacoRuntime {
  readonly attach: (
    host: HTMLElement,
    input: IdeMonacoAttachInput,
    onEvent: (event: IdeMonacoDocumentEvent) => void,
    onVim: (projection: IdeVimProjection) => void,
  ) => IdeMonacoMountedView
  readonly resources: () => IdeMonacoRuntimeResourceSnapshot
  readonly dispose: () => void
}

export class IdeMonacoRuntimeLoadError extends Schema.TaggedErrorClass<IdeMonacoRuntimeLoadError>()(
  "IdeMonacoRuntimeLoadError",
  {
    operation: Schema.String,
    reason: Schema.Literals(["aborted", "module_unavailable", "invalid_module"]),
    cause: Schema.Defect(),
  },
) {}

const moduleUrl = "openagents-app://renderer/ide-editor/editor.js"
const stylesheetUrl = "openagents-app://renderer/ide-editor/editor.css"
let runtimePromise: Promise<IdeMonacoRuntime> | null = null
let loadedRuntime: IdeMonacoRuntime | null = null
let stylesheet: HTMLLinkElement | null = null

const decodeRuntime = (value: unknown): IdeMonacoRuntime | null => {
  if (typeof value !== "object" || value === null) return null
  const candidate = value as Partial<IdeMonacoRuntime>
  return typeof candidate.attach === "function" &&
    typeof candidate.resources === "function" &&
    typeof candidate.dispose === "function"
    ? candidate as IdeMonacoRuntime
    : null
}

const ensureStylesheet = (): void => {
  if (stylesheet?.isConnected === true) return
  const existing = document.querySelector<HTMLLinkElement>('link[data-oa-monaco-style="true"]')
  if (existing !== null) {
    stylesheet = existing
    return
  }
  const link = document.createElement("link")
  link.rel = "stylesheet"
  link.href = stylesheetUrl
  link.dataset.oaMonacoStyle = "true"
  document.head.append(link)
  stylesheet = link
}

const importRuntime = async (): Promise<IdeMonacoRuntime> => {
  ensureStylesheet()
  const loaded: unknown = await import(/* @vite-ignore */ moduleUrl)
  const runtime = decodeRuntime((loaded as { runtime?: unknown }).runtime)
  if (runtime === null) throw new Error("The Monaco editor module did not export its bounded runtime.")
  loadedRuntime = runtime
  return runtime
}

export const loadIdeMonacoRuntime = Effect.fn("IdeMonacoRuntime.load")(
  function* (signal?: AbortSignal) {
    const aborted = (): boolean => signal?.aborted === true
    if (aborted()) {
      return yield* new IdeMonacoRuntimeLoadError({
        operation: "IdeMonacoRuntime.load",
        reason: "aborted",
        cause: "aborted before import",
      })
    }
    runtimePromise ??= importRuntime().catch((cause: unknown) => {
      runtimePromise = null
      throw cause
    })
    const runtime = yield* Effect.tryPromise({
      try: () => runtimePromise!,
      catch: cause => new IdeMonacoRuntimeLoadError({
        operation: "IdeMonacoRuntime.load",
        reason: "module_unavailable",
        cause,
      }),
    })
    if (aborted()) {
      return yield* new IdeMonacoRuntimeLoadError({
        operation: "IdeMonacoRuntime.load",
        reason: "aborted",
        cause: "aborted after import",
      })
    }
    return runtime
  },
)

export const ideMonacoRuntimeResources = (): IdeMonacoRuntimeResourceSnapshot => {
  if (loadedRuntime !== null) return loadedRuntime.resources()
  if (runtimePromise === null) {
    return IdeMonacoRuntimeResourceSnapshotSchema.make({
      state: "idle",
      modelCount: 0,
      viewCount: 0,
      workerCount: 0,
      listenerCount: 0,
      vimHandlerCount: 0,
    })
  }
  return {
    state: "loading",
    modelCount: 0,
    viewCount: 0,
    workerCount: 0,
    listenerCount: 0,
    vimHandlerCount: 0,
  }
}

export const decodeIdeMonacoAttachInput = Schema.decodeUnknownSync(IdeMonacoAttachInputSchema)
export const decodeIdeMonacoDocumentEvent = Schema.decodeUnknownSync(IdeMonacoDocumentEventSchema)
export const decodeIdeVimProjection = Schema.decodeUnknownSync(IdeVimProjectionSchema)
