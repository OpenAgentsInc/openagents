import { mkdirSync, readFileSync, renameSync, unlinkSync, writeFileSync } from "node:fs"
import { createHash } from "node:crypto"
import path from "node:path"

import { Context, Effect, Exit, Layer, Scope } from "effect"

import type { DesktopWorkspaceService } from "../workspace-service.ts"
import {
  IdeAgentEvidenceFactSchema,
  IdeAgentEvidenceStateSchema,
  IdeAgentCodeCommandResultSchema,
  IdeAgentCodeSnapshotSchema,
  decodeIdeAgentCodeCommand,
  decodeIdeAgentCodeSnapshot,
  emptyIdeAgentCodeSnapshot,
  type IdeAgentCodeCommand,
  type IdeAgentCodeCommandResult,
  type IdeAgentCodeSnapshot,
} from "./agent-code-contract.ts"
import { IdeLanguageRequestRefSchema, IdeLanguageRequestSchema } from "./language-contract.ts"
import {
  IdeDocumentGeneration as IdeLanguageDocumentGeneration,
  IdeDocumentRef as IdeLanguageDocumentRef,
  IdeMonacoModelVersion,
} from "./monaco-document-contract.ts"
import {
  IdeAttachmentRefSchema,
  IdeEvidenceRefSchema,
  IdeLanguageGenerationSchema,
  IdeTimestampSchema,
} from "./project-contract.ts"
import {
  IdeAgentAuthorityFailure,
  IdeAgentCodeBaseChanged,
  IdeAgentCodeCheckpointExpired,
  IdeAgentCodeInvalidInput,
  IdeAgentCodeInvariantViolation,
  IdeAgentCodeMissing,
  IdeAgentCodeProposalState,
  IdeAgentCodeService,
  IdeAgentCodeStaleGeneration,
  IdeAgentCodeStopped,
  makeIdeAgentCodeLayer,
  type IdeAgentCodeServiceShape,
  type IdeAgentCodeServiceError,
} from "./agent-code-service.ts"
import { makeIdeAgentWorkspaceAuthorityLayer } from "./agent-code-workspace-authority.ts"

export type IdeAgentCodeHost = Readonly<{
  workspaceGrantRef: string
  snapshot: () => Promise<IdeAgentCodeSnapshot>
  command: (value: unknown) => Promise<IdeAgentCodeCommandResult>
  dispose: () => Promise<void>
}>

const loadPersistedSnapshot = (
  persistencePath: string | null,
): Readonly<{ snapshot: IdeAgentCodeSnapshot; corrupt: boolean }> => {
  if (persistencePath === null) return { snapshot: emptyIdeAgentCodeSnapshot(), corrupt: false }
  try {
    const decoded = decodeIdeAgentCodeSnapshot(JSON.parse(readFileSync(persistencePath, "utf8")))
    return decoded === null
      ? { snapshot: emptyIdeAgentCodeSnapshot(), corrupt: true }
      : { snapshot: decoded.lifecycle === "stopped" ? emptyIdeAgentCodeSnapshot() : decoded, corrupt: false }
  } catch (error) {
    return (error as NodeJS.ErrnoException | null)?.code === "ENOENT"
      ? { snapshot: emptyIdeAgentCodeSnapshot(), corrupt: false }
      : { snapshot: emptyIdeAgentCodeSnapshot(), corrupt: true }
  }
}

const persistSnapshot = (persistencePath: string | null, snapshot: IdeAgentCodeSnapshot): void => {
  if (persistencePath === null) return
  mkdirSync(path.dirname(persistencePath), { recursive: true, mode: 0o700 })
  const temporary = `${persistencePath}.${process.pid}.${snapshot.revision}.tmp`
  try {
    writeFileSync(temporary, JSON.stringify(snapshot), { encoding: "utf8", flag: "wx", mode: 0o600 })
    renameSync(temporary, persistencePath)
  } catch {
    try { unlinkSync(temporary) } catch { /* best-effort temporary cleanup */ }
  }
}

const resultReason = (
  error: IdeAgentCodeServiceError,
): Extract<IdeAgentCodeCommandResult, { _tag: "Refused" }>["reason"] => {
  if (error instanceof IdeAgentCodeInvalidInput) return "invalid_input"
  if (error instanceof IdeAgentCodeStopped) return "stopped"
  if (error instanceof IdeAgentCodeStaleGeneration) return "stale_generation"
  if (error instanceof IdeAgentCodeProposalState) return "proposal_state"
  if (error instanceof IdeAgentCodeBaseChanged) return error.reason === "dirty_document" ? "dirty_document" : "base_changed"
  if (error instanceof IdeAgentCodeMissing) return error.resource === "manifest" ? "manifest_missing" : "proposal_missing"
  if (error instanceof IdeAgentCodeCheckpointExpired) return "checkpoint_expired"
  if (error instanceof IdeAgentAuthorityFailure) {
    switch (error.reason) {
      case "conflict": return "conflict"
      case "unsupported_policy": return "unsupported_policy"
      case "grant_revoked": return "grant_revoked"
      case "rollback_failed": return "rollback_failed"
      case "unavailable": return "unavailable"
    }
  }
  if (error instanceof IdeAgentCodeInvariantViolation) {
    return error.detail.includes("no project/worktree attachment") ? "unattached" : "conflict"
  }
  return "unavailable"
}

const resultMessage = (error: IdeAgentCodeServiceError): string => {
  if ("detail" in error && typeof error.detail === "string" && error.detail.trim() !== "") return error.detail
  if ("message" in error && typeof error.message === "string" && error.message.trim() !== "") return error.message
  return "The agent-code operation was refused."
}

const executeCommand = (
  service: IdeAgentCodeServiceShape,
  command: IdeAgentCodeCommand,
) => {
  switch (command._tag) {
    case "Attach": return service.attach(command.attachment)
    case "AssembleManifest": return service.assembleManifest(command.input)
    case "SubmitProposal": return service.submitProposal(command.input)
    case "BeginReview": return service.beginReview(command.input)
    case "Decide": return service.decide(command.decision, command.expectedAttachmentGeneration)
    case "Apply": return service.apply(command.input)
    case "Rebase": return service.rebase(command.input)
    case "Undo": return service.undo(command.input)
    case "Stop": return service.stop(command.reason)
  }
}

const observedEvidence = async (
  service: IdeAgentCodeServiceShape,
  workspace: DesktopWorkspaceService,
  snapshot: IdeAgentCodeSnapshot,
  proposalRef: string,
): Promise<IdeAgentCodeSnapshot> => {
  const proposal = snapshot.proposals.find(candidate => candidate.proposalRef === proposalRef)
  if (proposal === undefined || proposal.lifecycle._tag !== "Applied") return snapshot
  const observedAt = IdeTimestampSchema.make(new Date().toISOString())
  const applyRef = proposal.lifecycle.applyRef
  const postImageGeneration = Math.max(1, ...snapshot.backlinks
    .filter(backlink => backlink.proposalRef === proposal.proposalRef && backlink.resolution._tag === "Current")
    .map(backlink => backlink.resolution._tag === "Current" ? backlink.resolution.documentGeneration : 1))
  const evidenceSuffix = createHash("sha256").update(applyRef).digest("hex").slice(0, 24)
  const status = (() => {
    try {
      return workspace.gitStatus()
    } catch {
      return null
    }
  })()
  const paths = proposal.operations.flatMap(operation => operation._tag === "Delete"
    ? []
    : [operation._tag === "Rename" ? operation.targetPathRef : operation.pathRef])
  const diffs = paths.map(pathRef => {
    try {
      return workspace.gitDiff(pathRef)
    } catch {
      return null
    }
  })

  const languageResults: Array<Readonly<{
    capability: "diagnostics" | "format_document"
    response: Awaited<ReturnType<DesktopWorkspaceService["languageRequest"]>> | null
  }>> = []
  for (const [pathIndex, pathRef] of paths.entries()) {
    const language = /\.tsx?$/iu.test(pathRef) ? "typescript" : /\.jsx?$/iu.test(pathRef) ? "javascript" : null
    const opened = workspace.openDocument({ grantRef: workspace.grantRef, pathRef })
    if (language === null || opened.state !== "available") continue
    for (const capability of ["diagnostics", "format_document"] as const) {
      const operation = proposal.operations.find(candidate =>
        (candidate._tag === "Rename" ? candidate.targetPathRef : candidate.pathRef) === pathRef)
      if (operation === undefined) continue
      const requestSuffix = `${evidenceSuffix}.${pathIndex + 1}.${capability}`
      const response = await workspace.languageRequest(IdeLanguageRequestSchema.make({
          schemaVersion: "openagents.desktop.ide-language-request.v1",
          grantRef: workspace.grantRef,
          requestRef: IdeLanguageRequestRefSchema.make(`ide.language-request.agent-evidence.${requestSuffix}`),
          capability,
          projectRef: proposal.attachment.projectRef,
          rootRef: proposal.attachment.rootRef,
          worktreeRef: proposal.attachment.worktreeRef,
          attachmentRef: IdeAttachmentRefSchema.make(`ide.attachment.agent-evidence.${evidenceSuffix}`),
          attachmentGeneration: proposal.attachment.attachmentGeneration,
          languageGeneration: IdeLanguageGenerationSchema.make(1),
        documentRef: IdeLanguageDocumentRef.make(`ide.document.${evidenceSuffix.slice(0, 12)}.${pathIndex + 1}`),
          fileRef: operation.fileRef,
          pathRef,
        documentGeneration: IdeLanguageDocumentGeneration.make(postImageGeneration),
          documentVersion: IdeMonacoModelVersion.make(1),
          expectedServiceGeneration: null,
          requestedAt: observedAt,
          language,
          content: opened.document.content,
          position: { line: 1, column: 1, offset: 0 },
          range: { start: { line: 1, column: 1, offset: 0 }, end: { line: 1, column: 1, offset: 0 } },
          query: null,
          limit: 1_000,
          timeoutMs: 5_000,
        })).catch(() => null)
      languageResults.push({ capability, response })
    }
  }

  const languageState = (capability: "diagnostics" | "format_document") => {
    const results = languageResults.filter(result => result.capability === capability)
    if (results.length === 0) return IdeAgentEvidenceStateSchema.cases.Unavailable.make({
      observedAt,
      reason: "No supported current document admitted this language capability.",
    })
    if (results.some(result => result.response === null)) return IdeAgentEvidenceStateSchema.cases.Unavailable.make({
      observedAt,
      reason: "The host language service could not observe this capability.",
    })
    const observed = results.flatMap(result => result.response === null ? [] : [result.response])
    const rejected = observed.find(response => response._tag === "Rejected")
    if (rejected?._tag === "Rejected") return IdeAgentEvidenceStateSchema.cases.Unavailable.make({
      observedAt,
      reason: rejected.message,
    })
    const unavailable = observed.find(response => response._tag === "Result" &&
      !["Complete", "Partial", "Truncated"].includes(response.result.state._tag))
    if (unavailable?._tag === "Result") return IdeAgentEvidenceStateSchema.cases.Unavailable.make({
      observedAt,
      reason: `Language evidence was ${unavailable.result.state._tag.toLocaleLowerCase()}.`,
    })
    if (capability === "diagnostics") {
      const errors = observed.flatMap(response => response._tag === "Result"
        ? response.result.items.filter(item => item._tag === "Diagnostic" && item.severity === "error")
        : [])
      return errors.length === 0
        ? IdeAgentEvidenceStateSchema.cases.Passed.make({ observedAt, summary: `Host language service observed zero errors across ${observed.length} current document(s).` })
        : IdeAgentEvidenceStateSchema.cases.Failed.make({ observedAt, summary: `Host language service observed ${errors.length} error diagnostic(s).` })
    }
    const edits = observed.reduce((total, response) => total + (response._tag === "Result"
      ? response.result.items.filter(item => item._tag === "TextEdit").length
      : 0), 0)
    return IdeAgentEvidenceStateSchema.cases.Passed.make({ observedAt, summary: `Host language service observed ${edits} formatting edit(s); no edit was auto-applied.` })
  }

  const facts = [
    ["diagnostics", "language_service", languageState("diagnostics"), null],
    ["format", "language_service", languageState("format_document"), null],
    ["test", "task_service", IdeAgentEvidenceStateSchema.cases.Unavailable.make({ observedAt, reason: "No exact test command was admitted for this proposal." }), null],
    ["git_status", "git_service", status?.state === "available"
      ? IdeAgentEvidenceStateSchema.cases.Passed.make({ observedAt, summary: `Host Git status observed ${status.changes.length} change(s)${status.truncated ? " (truncated)" : ""}.` })
      : IdeAgentEvidenceStateSchema.cases.Unavailable.make({ observedAt, reason: "The WorkContext is not an available Git repository." }), null],
    ["git_diff", "git_service", diffs.length > 0 && diffs.every(diff => diff?.state === "available")
      ? IdeAgentEvidenceStateSchema.cases.Passed.make({ observedAt, summary: `Host Git diff observed ${diffs.length} applied path(s).` })
      : IdeAgentEvidenceStateSchema.cases.Unavailable.make({ observedAt, reason: "No complete bounded Git diff was available for every applied path." }), null],
    ["delivery", "delivery_service", IdeAgentEvidenceStateSchema.cases.Unavailable.make({ observedAt, reason: "Apply does not imply commit, push, pull request, or delivery." }), null],
    ["verification", "independent_reviewer", IdeAgentEvidenceStateSchema.cases.Unavailable.make({ observedAt, reason: "No independent reviewer has verified this post-image." }), null],
    ["acceptance", "owner", IdeAgentEvidenceStateSchema.cases.Unavailable.make({ observedAt, reason: "The owner has not accepted this post-image." }), null],
  ] as const
  let current = snapshot
  for (const [kind, observedBy, state, artifactRef] of facts) {
    current = await Effect.runPromise(service.recordEvidence(IdeAgentEvidenceFactSchema.make({
      evidenceRef: IdeEvidenceRefSchema.make(`ide.evidence.agent.${evidenceSuffix}.${kind}`),
      proposalRef: proposal.proposalRef,
      applyRef,
      postImageGeneration,
      kind,
      state,
      observedBy,
      artifactRef,
      commitRef: null,
      lineage: proposal.lineage,
    }), proposal.attachment.attachmentGeneration))
  }
  return current
}

export const openIdeAgentCodeHost = async (
  workspace: DesktopWorkspaceService,
  options: Readonly<{ persistencePath?: string | null }> = {},
): Promise<IdeAgentCodeHost> => {
  const persistencePath = options.persistencePath ?? null
  const recovered = loadPersistedSnapshot(persistencePath)
  const scope = await Effect.runPromise(Scope.make())
  const layer = makeIdeAgentCodeLayer(recovered.snapshot).pipe(
    Layer.provide(makeIdeAgentWorkspaceAuthorityLayer(workspace)),
  )
  const context = await Effect.runPromise(Layer.buildWithScope(layer, scope))
  const service = Context.get(context, IdeAgentCodeService)
  let disposed = false
  let corruptRecovery = recovered.corrupt

  const snapshot = async (): Promise<IdeAgentCodeSnapshot> => {
    if (disposed) return IdeAgentCodeSnapshotSchema.make({
      ...emptyIdeAgentCodeSnapshot(),
      lifecycle: "stopped",
    })
    return Effect.runPromise(service.snapshot()).catch(() => IdeAgentCodeSnapshotSchema.make({
      ...emptyIdeAgentCodeSnapshot(),
      lifecycle: "stopped",
    }))
  }

  const refused = async (
    reason: Extract<IdeAgentCodeCommandResult, { _tag: "Refused" }>["reason"],
    message: string,
  ): Promise<IdeAgentCodeCommandResult> => IdeAgentCodeCommandResultSchema.cases.Refused.make({
    reason,
    message: message.slice(0, 1_000),
    snapshot: await snapshot(),
  })

  const command = async (value: unknown): Promise<IdeAgentCodeCommandResult> => {
    if (disposed) return refused("stopped", "The project agent-code scope is closed.")
    const decoded = decodeIdeAgentCodeCommand(value)
    if (decoded === null) return refused("invalid_input", "The agent-code command did not match the schema boundary.")
    if (decoded._tag === "Attach" && decoded.attachment.grantRef !== workspace.grantRef) {
      return refused("wrong_attachment", "The attachment does not belong to the current workspace grant.")
    }
    if (corruptRecovery && decoded._tag !== "Attach") {
      return refused("corrupt_persistence", "Persisted agent-code state was corrupt. Reattach explicitly to start a clean generation.")
    }
    if (corruptRecovery && decoded._tag === "Attach") corruptRecovery = false
    const settled = await Effect.runPromise(executeCommand(service, decoded).pipe(Effect.match({
      onFailure: error => ({ ok: false as const, error }),
      onSuccess: next => ({ ok: true as const, next }),
    })))
    if (!settled.ok) return refused(resultReason(settled.error), resultMessage(settled.error))
    const next = decoded._tag === "Apply"
      ? await observedEvidence(service, workspace, settled.next, decoded.input.proposalRef)
      : settled.next
    persistSnapshot(persistencePath, next)
    return IdeAgentCodeCommandResultSchema.cases.Succeeded.make({ snapshot: next })
  }

  const dispose = async (): Promise<void> => {
    if (disposed) return
    const last = await snapshot()
    persistSnapshot(persistencePath, last)
    disposed = true
    await Effect.runPromise(Scope.close(scope, Exit.void))
  }

  return { workspaceGrantRef: workspace.grantRef, snapshot, command, dispose }
}
