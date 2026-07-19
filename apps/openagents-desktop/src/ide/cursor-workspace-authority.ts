import { createHash } from "node:crypto"

import { Effect, Layer, Schema } from "effect"

import type { DesktopWorkspaceDocument, DesktopWorkspaceDocumentResult } from "../workspace-contract.ts"
import type { DesktopWorkspaceService } from "../workspace-service.ts"
import {
  IdeCursorAppliedResultSchema,
  IdeCursorAuthorityFailure,
  IdeCursorDocumentAuthority,
  type IdeCursorDocumentAuthorityShape,
} from "./cursor-service.ts"
import {
  IdeCursorAnchorSchema,
  IdeCursorCandidateRefSchema,
  IdeCursorRequestRefSchema,
  IdeCursorSequenceSchema,
  type IdeCursorAnchor,
} from "./cursor-contract.ts"
import {
  IdeDocumentGenerationSchema,
  IdeDocumentRefSchema,
  IdeFileRefSchema,
  type IdeTextRange,
} from "./project-contract.ts"

const digestSchema = Schema.String.check(Schema.isPattern(/^sha256:[0-9a-f]{64}$/u))
const revisionSchema = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(192))

/**
 * This record separates the facts that main can prove from renderer-owned
 * Monaco state. Main proves the canonical path, refs, content, and revision.
 * The cursor contract and renderer admission retain the source versions.
 */
export const IdeCursorWorkspaceDocumentSchema = Schema.Struct({
  pathRef: IdeCursorAnchorSchema.fields.pathRef,
  fileRef: IdeFileRefSchema,
  documentRef: IdeDocumentRefSchema,
  documentGeneration: IdeDocumentGenerationSchema,
  revisionRef: revisionSchema,
  contentDigest: digestSchema,
  content: Schema.String.check(Schema.isMaxLength(1_000_000)),
}).annotate({ identifier: "IdeCursorWorkspaceDocument" })
export interface IdeCursorWorkspaceDocument extends Schema.Schema.Type<typeof IdeCursorWorkspaceDocumentSchema> {}

export const IdeCursorWorkspaceAcceptanceSchema = Schema.Struct({
  candidateRef: IdeCursorCandidateRefSchema,
  requestRef: IdeCursorRequestRefSchema,
  sequence: IdeCursorSequenceSchema,
  anchor: IdeCursorAnchorSchema,
  granularity: Schema.Literals(["word", "line", "all"]),
  pathRef: IdeCursorAnchorSchema.fields.pathRef,
  previousRevisionRef: revisionSchema,
  previousContentDigest: digestSchema,
  previousContent: Schema.String.check(Schema.isMaxLength(1_000_000)),
  acceptedRevisionRef: revisionSchema,
  acceptedContentDigest: digestSchema,
  acceptedContent: Schema.String.check(Schema.isMaxLength(1_000_000)),
  state: Schema.Literals(["accepted", "undone"]),
}).annotate({ identifier: "IdeCursorWorkspaceAcceptance" })
export interface IdeCursorWorkspaceAcceptance extends Schema.Schema.Type<typeof IdeCursorWorkspaceAcceptanceSchema> {}

const sha256 = (value: string): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`

const safeSuffix = (value: string): string =>
  createHash("sha256").update(value).digest("hex").slice(0, 32)

export const ideCursorWorkspaceFileRef = (pathRef: string) =>
  IdeFileRefSchema.make(`ide.file.workspace.${safeSuffix(pathRef)}`)

export const ideCursorWorkspaceDocumentRef = (pathRef: string) =>
  IdeDocumentRefSchema.make(`ide.document.workspace.${safeSuffix(pathRef)}`)

const failure = (
  operation: string,
  reason: IdeCursorAuthorityFailure["reason"],
  detail: string,
): IdeCursorAuthorityFailure => new IdeCursorAuthorityFailure({ operation, reason, detail: detail.slice(0, 2_000) })

const workspaceFailure = (
  operation: string,
  result: Exclude<DesktopWorkspaceDocumentResult, { readonly state: "available" } | { readonly state: "saved" }>,
): IdeCursorAuthorityFailure => {
  if (result.state === "conflict") {
    return failure(operation, "conflict", "The canonical document revision changed before the operation completed.")
  }
  return failure(
    operation,
    result.reason === "grant_revoked" || result.reason === "permission_denied" ? "unavailable" : "unavailable",
    result.message,
  )
}

const canonicalDocument = (
  workspace: DesktopWorkspaceService,
  pathRef: string,
  operation: string,
): Effect.Effect<DesktopWorkspaceDocument, IdeCursorAuthorityFailure> => Effect.gen(function* () {
  const result = yield* Effect.sync(() => workspace.openDocument({ grantRef: workspace.grantRef, pathRef }))
  if (result.state === "available" || result.state === "saved") return result.document
  return yield* Effect.fail(workspaceFailure(operation, result))
})

const projectedDocument = (
  anchor: IdeCursorAnchor,
  document: DesktopWorkspaceDocument,
): IdeCursorWorkspaceDocument => IdeCursorWorkspaceDocumentSchema.make({
  pathRef: document.pathRef,
  fileRef: ideCursorWorkspaceFileRef(document.pathRef),
  documentRef: ideCursorWorkspaceDocumentRef(document.pathRef),
  // DesktopWorkspaceService has no Monaco lifecycle authority. Retain the
  // admitted generation while main independently verifies the path and refs.
  documentGeneration: anchor.documentGeneration,
  revisionRef: document.revisionRef,
  contentDigest: sha256(document.content),
  content: document.content,
})

const offsetAt = (
  content: string,
  position: IdeCursorAnchor["selection"]["start"],
): number | null => {
  const starts = [0]
  for (let index = 0; index < content.length; index += 1) {
    if (content.charCodeAt(index) === 10) starts.push(index + 1)
  }
  const lineStart = starts[position.line - 1]
  if (lineStart === undefined) return null
  const nextLineStart = starts[position.line]
  let lineEnd = nextLineStart === undefined ? content.length : nextLineStart - 1
  if (lineEnd > lineStart && content.charCodeAt(lineEnd - 1) === 13) lineEnd -= 1
  const offset = lineStart + position.column - 1
  return offset <= lineEnd ? offset : null
}

const offsetsFor = (
  content: string,
  range: IdeTextRange,
): Readonly<{ start: number; end: number }> | null => {
  const start = offsetAt(content, range.start)
  const end = offsetAt(content, range.end)
  return start === null || end === null || end < start ? null : { start, end }
}

const wordPrefix = (text: string): string => {
  const match = /^[\t ]*(?:[\p{L}\p{N}_$]+|[^\p{L}\p{N}_$\s]+|\r?\n)/u.exec(text)
  return match?.[0] ?? text
}

const linePrefix = (text: string): string => {
  const newline = /\r?\n/u.exec(text)
  return newline === null ? text : text.slice(0, newline.index + newline[0].length)
}

export const ideCursorAcceptedText = (
  text: string,
  granularity: "word" | "line" | "all",
): string => {
  switch (granularity) {
    case "word": return wordPrefix(text)
    case "line": return linePrefix(text)
    case "all": return text
  }
}

const sameAnchorIdentity = (left: IdeCursorAnchor, right: IdeCursorAnchor): boolean =>
  left.projectRef === right.projectRef &&
  left.rootRef === right.rootRef &&
  left.worktreeRef === right.worktreeRef &&
  left.attachmentRef === right.attachmentRef &&
  left.attachmentGeneration === right.attachmentGeneration &&
  left.sessionRef === right.sessionRef &&
  left.sourceDocumentRef === right.sourceDocumentRef &&
  left.sourceDocumentGeneration === right.sourceDocumentGeneration &&
  left.fileRef === right.fileRef &&
  left.documentRef === right.documentRef &&
  left.documentGeneration === right.documentGeneration &&
  left.documentSequence === right.documentSequence &&
  left.modelVersion === right.modelVersion &&
  left.selectionVersion === right.selectionVersion &&
  left.pathRef === right.pathRef &&
  left.selection.start.line === right.selection.start.line &&
  left.selection.start.column === right.selection.start.column &&
  left.selection.end.line === right.selection.end.line &&
  left.selection.end.column === right.selection.end.column &&
  left.contentDigest === right.contentDigest

export const makeIdeCursorWorkspaceAuthority = (
  workspace: DesktopWorkspaceService,
): IdeCursorDocumentAuthorityShape => {
    const accepted = new Map<string, IdeCursorWorkspaceAcceptance>()

    const validate = Effect.fn("IdeCursorWorkspaceAuthority.validate")(function* (anchor: IdeCursorAnchor) {
      const document = yield* canonicalDocument(workspace, anchor.pathRef, "IdeCursorWorkspaceAuthority.validate")
      const projected = projectedDocument(anchor, document)
      if (document.pathRef !== anchor.pathRef || projected.fileRef !== anchor.fileRef ||
        projected.documentRef !== anchor.documentRef) {
        return yield* Effect.fail(failure(
          "IdeCursorWorkspaceAuthority.validate",
          "stale",
          "The path-derived file or document identity does not match the canonical workspace document.",
        ))
      }
      if (projected.contentDigest !== anchor.contentDigest) {
        return yield* Effect.fail(failure(
          "IdeCursorWorkspaceAuthority.validate",
          "stale",
          "The canonical document content does not match the candidate base digest.",
        ))
      }
      if (offsetsFor(document.content, anchor.selection) === null) {
        return yield* Effect.fail(failure(
          "IdeCursorWorkspaceAuthority.validate",
          "stale",
          "The admitted selection is outside the canonical document.",
        ))
      }
    })

    const accept: IdeCursorDocumentAuthorityShape["accept"] = Effect.fn(
      "IdeCursorWorkspaceAuthority.accept",
    )(function* (candidate, granularity) {
      if (candidate._tag !== "Completion" && candidate._tag !== "NextEdit") {
        return yield* Effect.fail(failure(
          "IdeCursorWorkspaceAuthority.accept",
          "unavailable",
          "Only completion and next-edit candidates can mutate one canonical document directly.",
        ))
      }
      if (candidate._tag === "NextEdit" && candidate.targetPathRef !== candidate.anchor.pathRef) {
        return yield* Effect.fail(failure(
          "IdeCursorWorkspaceAuthority.accept",
          "stale",
          "A cross-file next edit requires a new target-document anchor or an IDE-08 proposal.",
        ))
      }
      const previousAcceptance = accepted.get(candidate.candidateRef)
      if (previousAcceptance?.state === "accepted") {
        return yield* Effect.fail(failure(
          "IdeCursorWorkspaceAuthority.accept",
          "conflict",
          "This candidate already owns an active canonical undo boundary.",
        ))
      }
      yield* validate(candidate.anchor)
      const current = yield* canonicalDocument(workspace, candidate.anchor.pathRef, "IdeCursorWorkspaceAuthority.accept")
      const previousDigest = sha256(current.content)
      if (previousDigest !== candidate.anchor.contentDigest) {
        return yield* Effect.fail(failure(
          "IdeCursorWorkspaceAuthority.accept",
          "stale",
          "The canonical document changed after candidate validation.",
        ))
      }
      const offsets = offsetsFor(current.content, candidate.replace)
      if (offsets === null) {
        return yield* Effect.fail(failure(
          "IdeCursorWorkspaceAuthority.accept",
          "stale",
          "The candidate replacement range is outside the canonical document.",
        ))
      }
      const inserted = ideCursorAcceptedText(candidate.text, granularity)
      const nextContent = `${current.content.slice(0, offsets.start)}${inserted}${current.content.slice(offsets.end)}`
      const result = yield* Effect.sync(() => workspace.saveDocument({
        grantRef: workspace.grantRef,
        pathRef: candidate.anchor.pathRef,
        content: nextContent,
        expectedRevisionRef: current.revisionRef,
      }))
      if (result.state !== "saved") {
        return yield* Effect.fail(result.state === "available"
          ? failure(
              "IdeCursorWorkspaceAuthority.accept",
              "unavailable",
              "The workspace save boundary returned a read result instead of a saved post-image.",
            )
          : workspaceFailure("IdeCursorWorkspaceAuthority.accept", result))
      }
      const acceptedDigest = sha256(result.document.content)
      if (result.document.pathRef !== candidate.anchor.pathRef || acceptedDigest !== sha256(nextContent)) {
        return yield* Effect.fail(failure(
          "IdeCursorWorkspaceAuthority.accept",
          "conflict",
          "The canonical save post-image did not match the deterministic replacement.",
        ))
      }
      const receipt = IdeCursorWorkspaceAcceptanceSchema.make({
        candidateRef: candidate.candidateRef,
        requestRef: candidate.requestRef,
        sequence: candidate.sequence,
        anchor: candidate.anchor,
        granularity,
        pathRef: candidate.anchor.pathRef,
        previousRevisionRef: current.revisionRef,
        previousContentDigest: previousDigest,
        previousContent: current.content,
        acceptedRevisionRef: result.document.revisionRef,
        acceptedContentDigest: acceptedDigest,
        acceptedContent: result.document.content,
        state: "accepted",
      })
      accepted.set(candidate.candidateRef, receipt)
      return IdeCursorAppliedResultSchema.make({
        previousContentDigest: previousDigest,
        resultContentDigest: acceptedDigest,
      })
    })

    const undo: IdeCursorDocumentAuthorityShape["undo"] = Effect.fn(
      "IdeCursorWorkspaceAuthority.undo",
    )(function* (candidate) {
      const acceptance = accepted.get(candidate.candidateRef)
      if (acceptance === undefined || acceptance.state !== "accepted") {
        return yield* Effect.fail(failure(
          "IdeCursorWorkspaceAuthority.undo",
          "conflict",
          "This candidate has no active canonical acceptance to undo.",
        ))
      }
      if (candidate.requestRef !== acceptance.requestRef || candidate.sequence !== acceptance.sequence ||
        !sameAnchorIdentity(candidate.anchor, acceptance.anchor)) {
        return yield* Effect.fail(failure(
          "IdeCursorWorkspaceAuthority.undo",
          "stale",
          "The undo candidate identity does not match the accepted canonical edit.",
        ))
      }
      const current = yield* canonicalDocument(workspace, acceptance.pathRef, "IdeCursorWorkspaceAuthority.undo")
      if (current.revisionRef !== acceptance.acceptedRevisionRef ||
        sha256(current.content) !== acceptance.acceptedContentDigest ||
        current.content !== acceptance.acceptedContent) {
        return yield* Effect.fail(failure(
          "IdeCursorWorkspaceAuthority.undo",
          "conflict",
          "The accepted post-image changed, so this exact undo boundary is stale.",
        ))
      }
      const result = yield* Effect.sync(() => workspace.saveDocument({
        grantRef: workspace.grantRef,
        pathRef: acceptance.pathRef,
        content: acceptance.previousContent,
        expectedRevisionRef: acceptance.acceptedRevisionRef,
      }))
      if (result.state !== "saved") {
        return yield* Effect.fail(result.state === "available"
          ? failure(
              "IdeCursorWorkspaceAuthority.undo",
              "unavailable",
              "The workspace undo boundary returned a read result instead of a saved post-image.",
            )
          : workspaceFailure("IdeCursorWorkspaceAuthority.undo", result))
      }
      const restoredDigest = sha256(result.document.content)
      if (restoredDigest !== acceptance.previousContentDigest || result.document.content !== acceptance.previousContent) {
        return yield* Effect.fail(failure(
          "IdeCursorWorkspaceAuthority.undo",
          "conflict",
          "The canonical undo post-image does not match the exact preimage.",
        ))
      }
      accepted.set(candidate.candidateRef, IdeCursorWorkspaceAcceptanceSchema.make({
        ...acceptance,
        state: "undone",
      }))
      return IdeCursorAppliedResultSchema.make({
        previousContentDigest: acceptance.acceptedContentDigest,
        resultContentDigest: restoredDigest,
      })
    })

  return IdeCursorDocumentAuthority.of({ validate, accept, undo })
}

export const makeIdeCursorWorkspaceAuthorityLayer = (
  workspace: DesktopWorkspaceService,
): Layer.Layer<IdeCursorDocumentAuthority> => Layer.succeed(
  IdeCursorDocumentAuthority,
  makeIdeCursorWorkspaceAuthority(workspace),
)
