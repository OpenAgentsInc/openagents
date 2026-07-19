import type { IdeAgentCodeSnapshot, IdeAgentProposal, IdeAgentProposalOperation } from "../../ide/agent-code-contract.ts"
import {
  IdeReviewRefSchema,
  IdeReviewSourceSchema,
  IdeReviewVersionRefSchema,
  type IdeDocumentGeneration,
  type IdeReviewAction,
  type IdeReviewSource,
} from "../../ide/project-contract.ts"

const lines = (content: string): ReadonlyArray<string> => {
  if (content === "") return []
  const normalized = content.replace(/\r\n/gu, "\n")
  const withoutTerminal = normalized.endsWith("\n") ? normalized.slice(0, -1) : normalized
  return withoutTerminal === "" ? [] : withoutTerminal.split("\n")
}

const fullReplacementPatch = (
  oldPath: string,
  newPath: string,
  before: string,
  after: string,
  kind: IdeAgentProposalOperation["_tag"],
): string => {
  const oldLines = lines(before)
  const newLines = lines(after)
  const metadata = kind === "Create"
    ? "new file mode 100644\n"
    : kind === "Delete"
      ? "deleted file mode 100644\n"
      : kind === "Rename"
        ? `similarity index 100%\nrename from ${oldPath}\nrename to ${newPath}\n`
        : ""
  if (kind === "Rename" && before === after) {
    return `diff --git a/${oldPath} b/${newPath}\n${metadata}`
  }
  const oldHeader = kind === "Create" ? "/dev/null" : `a/${oldPath}`
  const newHeader = kind === "Delete" ? "/dev/null" : `b/${newPath}`
  const hunk = [
    `@@ -${oldLines.length === 0 ? "0,0" : `1,${oldLines.length}`} +${newLines.length === 0 ? "0,0" : `1,${newLines.length}`} @@`,
    ...oldLines.map(line => `-${line}`),
    ...newLines.map(line => `+${line}`),
  ].join("\n")
  return `diff --git a/${oldPath} b/${newPath}\n${metadata}--- ${oldHeader}\n+++ ${newHeader}\n${hunk}\n`
}

export const agentProposalPatch = (proposal: IdeAgentProposal): string => proposal.operations.map(operation => {
  switch (operation._tag) {
    case "Create":
      return fullReplacementPatch(operation.pathRef, operation.pathRef, "", operation.content, operation._tag)
    case "Edit":
      return fullReplacementPatch(operation.pathRef, operation.pathRef, operation.base.content ?? "", operation.targetContent, operation._tag)
    case "Rename":
      return fullReplacementPatch(operation.pathRef, operation.targetPathRef, operation.base.content ?? "", operation.base.content ?? "", operation._tag)
    case "Delete":
      return fullReplacementPatch(operation.pathRef, operation.pathRef, operation.base.content ?? "", "", operation._tag)
  }
}).join("\n")

const lifecycleFor = (proposal: IdeAgentProposal): IdeReviewSource["lifecycle"] => {
  switch (proposal.lifecycle._tag) {
    case "RebaseRequired":
      return { _tag: "Stale", reason: "base_moved", refreshable: true }
    case "Cancelled":
    case "Superseded":
    case "Failed":
      return { _tag: "Unavailable", reason: "generation_replaced", refreshable: false }
    default:
      return { _tag: "Ready" }
  }
}

const actionsFor = (proposal: IdeAgentProposal): ReadonlyArray<IdeReviewAction> => {
  switch (proposal.lifecycle._tag) {
    case "Pending":
    case "Reviewing": return ["open", "select", "expand_context", "collapse_context", "accept", "reject"]
    case "Accepted": return ["open", "select", "expand_context", "collapse_context", "apply", "reject"]
    case "Applied": return ["open", "select", "expand_context", "collapse_context", "undo"]
    default: return ["open", "expand_context", "collapse_context"]
  }
}

export const agentProposalReviewSource = (
  proposal: IdeAgentProposal,
  currentDocumentGeneration: IdeDocumentGeneration | null = null,
): IdeReviewSource | null => {
  const patch = agentProposalPatch(proposal)
  if (patch.length === 0 || patch.length > 4 * 1024 * 1024) return null
  const first = proposal.operations[0]!
  const single = proposal.operations.length === 1
  const baseBytes = proposal.operations.reduce((total, operation) => total + new TextEncoder().encode(operation.base.content ?? "").byteLength, 0)
  const targetBytes = proposal.operations.reduce((total, operation) => total + new TextEncoder().encode(
    operation._tag === "Create" ? operation.content : operation._tag === "Edit" ? operation.targetContent : operation._tag === "Delete" ? "" : operation.base.content ?? "",
  ).byteLength, 0)
  const suffix = proposal.proposalRef.slice("ide.proposal.".length)
  return IdeReviewSourceSchema.cases.AgentProposal.make({
    schemaVersion: "openagents.desktop.ide-review-source.v1",
    reviewRef: IdeReviewRefSchema.make(`ide.review.agent.${suffix}`),
    projectRef: proposal.attachment.projectRef,
    rootRef: proposal.attachment.rootRef,
    worktreeRef: proposal.attachment.worktreeRef,
    fileRef: single ? first.fileRef : null,
    documentRef: single && "documentRef" in first ? first.documentRef : null,
    pathRef: single ? first.pathRef : null,
    scope: single ? "single_file" : "aggregate",
    base: {
      label: proposal.operations.length === 1 ? "Exact proposal base" : `${proposal.operations.length} exact proposal bases`,
      versionRef: IdeReviewVersionRefSchema.make(`ide.review-version.agent-base.${suffix}`),
      generation: Math.max(1, first.base.documentGeneration ?? proposal.attachment.attachmentGeneration),
      encoding: first.base.encoding === "none" ? "unknown" : first.base.encoding,
      lineEnding: first.base.lineEnding,
      content: { _tag: "Available", redacted: false, bytes: baseBytes },
    },
    target: {
      label: "Agent proposal",
      versionRef: IdeReviewVersionRefSchema.make(`ide.review-version.agent-target.${suffix}`),
      generation: Math.max(1, (first.base.documentGeneration ?? proposal.attachment.attachmentGeneration) + 1),
      encoding: first.policy.encoding === "preserve"
        ? first.base.encoding === "none" ? "unknown" : first.base.encoding
        : first.policy.encoding,
      lineEnding: first.policy.lineEnding === "preserve" ? first.base.lineEnding : first.policy.lineEnding,
      content: { _tag: "Available", redacted: false, bytes: targetBytes },
    },
    patch,
    language: single ? first.pathRef.split(".").at(-1) ?? null : null,
    origin: "agent",
    allowedActions: actionsFor(proposal),
    lifecycle: lifecycleFor(proposal),
    proposalRef: proposal.proposalRef,
    attachmentGeneration: proposal.attachment.attachmentGeneration,
    proposalBaseDocumentGeneration: single ? first.base.documentGeneration : null,
    currentDocumentGeneration,
  })
}

export const selectedAgentProposal = (
  snapshot: IdeAgentCodeSnapshot,
  proposalRef: string | null,
): IdeAgentProposal | null => {
  if (proposalRef !== null) {
    const selected = snapshot.proposals.find(proposal => proposal.proposalRef === proposalRef)
    if (selected !== undefined) return selected
  }
  return [...snapshot.proposals].reverse().find(proposal => !["Rejected", "Cancelled", "Superseded", "Failed"].includes(proposal.lifecycle._tag)) ?? null
}
