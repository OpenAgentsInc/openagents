import { createHash } from "node:crypto"
import path from "node:path"

import { Effect, Layer } from "effect"

import type { DesktopWorkspaceService } from "../workspace-service.ts"
import {
  IdeAgentAuthorityFailure,
  IdeAgentAuthorityFileSchema,
  IdeAgentAuthorityPostImageSchema,
  IdeAgentAuthoritySnapshotSchema,
  IdeAgentDocumentAuthority,
  type IdeAgentAuthorityFile,
  type IdeAgentAuthorityPostImage,
  type IdeAgentDocumentAuthorityShape,
} from "./agent-code-service.ts"
import type { IdeAgentPreimage, IdeAgentProposalOperation } from "./agent-code-contract.ts"
import {
  IdeDiskRevisionRefSchema,
  IdeDocumentGenerationSchema,
  IdeDocumentRefSchema,
  IdeFileRefSchema,
} from "./project-contract.ts"

const sha256 = (value: string): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`

const safeSuffix = (value: string): string => createHash("sha256").update(value).digest("hex").slice(0, 32)

const secretPath = (pathRef: string): boolean => pathRef.split("/").some(segment =>
  /^\.env(?:\.|$)|(?:^|[-_.])(?:id_rsa|id_ed25519|credentials|secrets?|tokens?|keychain)(?:[-_.]|$)|\.(?:key|p12|pem|pfx)$/iu.test(segment))

const secretContent = (content: string): boolean =>
  /-----BEGIN(?: [A-Z]+)? PRIVATE KEY-----|(?:github_pat|gh[pousr]_|sk-|AKIA)[A-Za-z0-9_\-]{8,}|authorization\s*:\s*bearer\s+\S+|(?:password|secret|token)\s*[:=]\s*[^\s]+/iu.test(content)

const authorityFailure = (
  operation: string,
  reason: IdeAgentAuthorityFailure["reason"],
  message: string,
): IdeAgentAuthorityFailure => new IdeAgentAuthorityFailure({ operation, reason, message: message.slice(0, 1_000) })

const lineEndingFor = (content: string): "lf" | "crlf" | "mixed" | "none" => {
  const crlf = (content.match(/\r\n/gu) ?? []).length
  const lf = (content.match(/(?<!\r)\n/gu) ?? []).length
  if (crlf > 0 && lf > 0) return "mixed"
  if (crlf > 0) return "crlf"
  if (lf > 0) return "lf"
  return "none"
}

const unavailableReason = (
  pathRef: string,
  reason: string,
): "grant_revoked" | "permission_denied" | "binary" | "secret" | "private" | "too_large" | "unsupported" | "unavailable" => {
  if (secretPath(pathRef)) return "secret"
  switch (reason) {
    case "grant_revoked": return "grant_revoked"
    case "permission_denied": return "permission_denied"
    case "binary": return "binary"
    case "too_large": return "too_large"
    case "unsupported_encoding": return "unsupported"
    case "invalid_ref": return "private"
    default: return "unavailable"
  }
}

const entryRevision = (
  workspace: DesktopWorkspaceService,
  pathRef: string,
): string | null => {
  const parentRef = path.posix.dirname(pathRef) === "." ? "" : path.posix.dirname(pathRef)
  let offset = 0
  for (let pageIndex = 0; pageIndex < 100; pageIndex += 1) {
    const page = workspace.tree({ directoryRef: parentRef, offset, limit: 200 })
    if (page.state !== "available") return null
    const entry = page.entries.find(candidate => candidate.pathRef === pathRef)
    if (entry !== undefined) return entry.revisionRef
    if (page.nextOffset === null) return null
    offset = page.nextOffset
  }
  return null
}

export const makeIdeAgentWorkspaceAuthorityLayer = (
  workspace: DesktopWorkspaceService,
): Layer.Layer<IdeAgentDocumentAuthority> => {
  const generations = new Map<string, number>()

  const projectFile = (pathRef: string): IdeAgentAuthorityFile | null | ReturnType<typeof IdeAgentAuthoritySnapshotSchema.cases.Unavailable.make> => {
    const opened = workspace.openDocument({ grantRef: workspace.grantRef, pathRef })
    if (opened.state === "unavailable") {
      if (opened.reason === "missing") return null
      return IdeAgentAuthoritySnapshotSchema.cases.Unavailable.make({
        pathRef,
        reason: unavailableReason(pathRef, opened.reason),
        message: opened.message,
      })
    }
    const document = opened.state === "conflict" ? opened.current : opened.document
    if (secretContent(document.content)) {
      return IdeAgentAuthoritySnapshotSchema.cases.Unavailable.make({
        pathRef,
        reason: "secret",
        message: "Secret-shaped document content is withheld from agent proposal authority.",
      })
    }
    const generation = generations.get(pathRef) ?? 1
    return IdeAgentAuthorityFileSchema.make({
      pathRef: document.pathRef,
      fileRef: IdeFileRefSchema.make(`ide.file.workspace.${safeSuffix(document.pathRef)}`),
      documentRef: IdeDocumentRefSchema.make(`ide.document.workspace.${safeSuffix(document.pathRef)}`),
      documentGeneration: IdeDocumentGenerationSchema.make(generation),
      diskRevisionRef: IdeDiskRevisionRefSchema.make(`ide.disk-revision.workspace.${safeSuffix(document.revisionRef)}`),
      content: document.content,
      contentDigest: sha256(document.content),
      encoding: document.encoding,
      lineEnding: document.lineEnding,
      mode: "regular",
      dirty: false,
      symlink: false,
      contentClass: "text",
    })
  }

  const snapshot: IdeAgentDocumentAuthorityShape["snapshot"] = pathRef => Effect.sync(() => {
    const projected = projectFile(pathRef)
    if (projected === null) return IdeAgentAuthoritySnapshotSchema.cases.Missing.make({ pathRef })
    return "_tag" in projected
      ? projected
      : IdeAgentAuthoritySnapshotSchema.cases.File.make({ file: projected })
  })

  const postImage = (
    operation: IdeAgentProposalOperation,
    pathRef: string,
  ): Effect.Effect<IdeAgentAuthorityPostImage, IdeAgentAuthorityFailure> => Effect.gen(function* () {
    const observed = yield* snapshot(pathRef)
    if (observed._tag === "Unavailable") {
      return yield* Effect.fail(authorityFailure(
        "IdeAgentWorkspaceAuthority.postImage",
        observed.reason === "grant_revoked" ? "grant_revoked" : "unavailable",
        observed.message,
      ))
    }
    if (observed._tag === "Missing") {
      return IdeAgentAuthorityPostImageSchema.make({
        operationRef: operation.operationRef,
        pathRef,
        diskRevisionRef: null,
        contentDigest: null,
        documentRef: null,
        documentGeneration: null,
        encoding: null,
        lineEnding: null,
        mode: null,
      })
    }
    return IdeAgentAuthorityPostImageSchema.make({
      operationRef: operation.operationRef,
      pathRef,
      diskRevisionRef: observed.file.diskRevisionRef,
      contentDigest: observed.file.contentDigest,
      documentRef: observed.file.documentRef,
      documentGeneration: observed.file.documentGeneration,
      encoding: observed.file.encoding,
      lineEnding: observed.file.lineEnding,
      mode: observed.file.mode,
    })
  })

  const apply: IdeAgentDocumentAuthorityShape["apply"] = operation => Effect.gen(function* () {
    if (operation.policy.mode !== "preserve" && operation.policy.mode !== "regular") {
      return yield* Effect.fail(authorityFailure(
        "IdeAgentWorkspaceAuthority.apply",
        "unsupported_policy",
        "Executable-mode mutation is not admitted by the text document authority.",
      ))
    }
    if (operation.policy.symlink !== "refuse") {
      return yield* Effect.fail(authorityFailure(
        "IdeAgentWorkspaceAuthority.apply",
        "unsupported_policy",
        "Symlink target mutation is not admitted by the workspace authority.",
      ))
    }
    switch (operation._tag) {
      case "Create": {
        if (operation.policy.encoding === "utf-8-bom") {
          return yield* Effect.fail(authorityFailure(
            "IdeAgentWorkspaceAuthority.apply.Create",
            "unsupported_policy",
            "Save As currently admits UTF-8 creation; UTF-8 BOM creation is explicit but unavailable.",
          ))
        }
        const result = workspace.saveDocumentAs({
          grantRef: workspace.grantRef,
          pathRef: operation.pathRef,
          content: operation.content,
        })
        if (result.state !== "saved") {
          return yield* Effect.fail(authorityFailure(
            "IdeAgentWorkspaceAuthority.apply.Create",
            result.state === "unavailable" && result.reason === "grant_revoked" ? "grant_revoked" : "conflict",
            result.state === "unavailable" ? result.message : "Create target changed before apply.",
          ))
        }
        generations.set(operation.pathRef, 1)
        return yield* postImage(operation, operation.pathRef)
      }
      case "Edit": {
        const current = workspace.openDocument({ grantRef: workspace.grantRef, pathRef: operation.pathRef })
        if (current.state !== "available") {
          return yield* Effect.fail(authorityFailure(
            "IdeAgentWorkspaceAuthority.apply.Edit",
            current.state === "unavailable" && current.reason === "grant_revoked" ? "grant_revoked" : "conflict",
            current.state === "unavailable" ? current.message : "Edit base changed before apply.",
          ))
        }
        const result = workspace.saveDocument({
          grantRef: workspace.grantRef,
          pathRef: operation.pathRef,
          content: operation.targetContent,
          expectedRevisionRef: current.document.revisionRef,
        })
        if (result.state !== "saved") {
          return yield* Effect.fail(authorityFailure(
            "IdeAgentWorkspaceAuthority.apply.Edit",
            result.state === "unavailable" && result.reason === "grant_revoked" ? "grant_revoked" : "conflict",
            result.state === "unavailable" ? result.message : "Edit base changed during canonical save.",
          ))
        }
        generations.set(operation.pathRef, (generations.get(operation.pathRef) ?? 1) + 1)
        return yield* postImage(operation, operation.pathRef)
      }
      case "Rename": {
        const sourceParent = path.posix.dirname(operation.pathRef)
        const targetParent = path.posix.dirname(operation.targetPathRef)
        if (sourceParent !== targetParent) {
          return yield* Effect.fail(authorityFailure(
            "IdeAgentWorkspaceAuthority.apply.Rename",
            "unsupported_policy",
            "Cross-directory rename requires the later SCM/workspace transaction packet.",
          ))
        }
        const revisionRef = entryRevision(workspace, operation.pathRef)
        if (revisionRef === null) {
          return yield* Effect.fail(authorityFailure(
            "IdeAgentWorkspaceAuthority.apply.Rename", "conflict", "Rename source changed before apply.",
          ))
        }
        const result = workspace.renameEntry({
          pathRef: operation.pathRef,
          name: path.posix.basename(operation.targetPathRef),
          expectedRevisionRef: revisionRef,
        })
        if (result.state !== "renamed") {
          return yield* Effect.fail(authorityFailure(
            "IdeAgentWorkspaceAuthority.apply.Rename",
            result.state === "permission_denied" ? "grant_revoked" : "conflict",
            "Rename source or target changed before canonical mutation.",
          ))
        }
        const generation = (generations.get(operation.pathRef) ?? 1) + 1
        generations.delete(operation.pathRef)
        generations.set(operation.targetPathRef, generation)
        return yield* postImage(operation, operation.targetPathRef)
      }
      case "Delete": {
        const revisionRef = entryRevision(workspace, operation.pathRef)
        if (revisionRef === null) {
          return yield* Effect.fail(authorityFailure(
            "IdeAgentWorkspaceAuthority.apply.Delete", "conflict", "Delete source changed before apply.",
          ))
        }
        const result = workspace.deleteEntry({ pathRef: operation.pathRef, expectedRevisionRef: revisionRef })
        if (result.state !== "deleted") {
          return yield* Effect.fail(authorityFailure(
            "IdeAgentWorkspaceAuthority.apply.Delete",
            result.state === "permission_denied" ? "grant_revoked" : "conflict",
            "Delete source changed before canonical mutation.",
          ))
        }
        generations.delete(operation.pathRef)
        return yield* postImage(operation, operation.pathRef)
      }
    }
  })

  const restore: IdeAgentDocumentAuthorityShape["restore"] = (operation, preimage, _postImage) => Effect.gen(function* () {
    const postPath = operation._tag === "Rename" ? operation.targetPathRef : operation.pathRef
    if (preimage._tag === "Missing") {
      const revisionRef = entryRevision(workspace, postPath)
      if (revisionRef !== null) {
        const result = workspace.deleteEntry({ pathRef: postPath, expectedRevisionRef: revisionRef })
        if (result.state !== "deleted") {
          return yield* Effect.fail(authorityFailure(
            "IdeAgentWorkspaceAuthority.restore.Missing", "rollback_failed", "Created post-image could not be removed.",
          ))
        }
      }
      generations.delete(postPath)
      return yield* postImage(operation, preimage.pathRef)
    }
    if (operation._tag === "Rename") {
      const revisionRef = entryRevision(workspace, postPath)
      if (revisionRef === null || path.posix.dirname(postPath) !== path.posix.dirname(preimage.pathRef)) {
        return yield* Effect.fail(authorityFailure(
          "IdeAgentWorkspaceAuthority.restore.Rename", "rollback_failed", "Renamed post-image cannot be restored exactly.",
        ))
      }
      const renamed = workspace.renameEntry({
        pathRef: postPath,
        name: path.posix.basename(preimage.pathRef),
        expectedRevisionRef: revisionRef,
      })
      if (renamed.state !== "renamed") {
        return yield* Effect.fail(authorityFailure(
          "IdeAgentWorkspaceAuthority.restore.Rename", "rollback_failed", "Rename rollback was refused.",
        ))
      }
      generations.delete(postPath)
      generations.set(preimage.pathRef, (generations.get(postPath) ?? 1) + 1)
      return yield* postImage(operation, preimage.pathRef)
    }
    const observed = workspace.openDocument({ grantRef: workspace.grantRef, pathRef: preimage.pathRef })
    const restored = observed.state === "available"
      ? workspace.saveDocument({
          grantRef: workspace.grantRef,
          pathRef: preimage.pathRef,
          content: preimage.content,
          expectedRevisionRef: observed.document.revisionRef,
        })
      : workspace.saveDocumentAs({
          grantRef: workspace.grantRef,
          pathRef: preimage.pathRef,
          content: preimage.content,
        })
    if (restored.state !== "saved") {
      return yield* Effect.fail(authorityFailure(
        "IdeAgentWorkspaceAuthority.restore.File", "rollback_failed", "Checkpoint preimage could not be restored.",
      ))
    }
    generations.set(preimage.pathRef, (generations.get(preimage.pathRef) ?? 1) + 1)
    return yield* postImage(operation, preimage.pathRef)
  })

  return Layer.succeed(IdeAgentDocumentAuthority, IdeAgentDocumentAuthority.of({ snapshot, apply, restore }))
}
