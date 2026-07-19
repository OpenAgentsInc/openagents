import { createHash } from "node:crypto"

import { Context, Effect, Layer, Ref, Schema, Semaphore } from "effect"

import {
  IdeAgentApplyRefSchema,
  IdeAgentApplyInputSchema,
  IdeAgentApplyReceiptSchema,
  IdeAgentAttachmentSchema,
  IdeAgentBacklinkRefSchema,
  IdeAgentBacklinkSchema,
  IdeAgentCheckpointSchema,
  IdeAgentCodeReceiptSchema,
  IdeAgentCodeSnapshotSchema,
  IdeAgentContextAssemblyInputSchema,
  IdeAgentDecisionSchema,
  IdeAgentEvidenceFactSchema,
  IdeAgentEvidenceStateSchema,
  IdeAgentOperationRefSchema,
  IdeAgentPreimageSchema,
  IdeAgentProposalInputSchema,
  IdeAgentProposalSchema,
  IdeAgentRebaseInputSchema,
  IdeAgentReviewInputSchema,
  IdeAgentUndoInputSchema,
  IdeAgentUndoRefSchema,
  IdeAgentUndoReceiptSchema,
  emptyIdeAgentCodeSnapshot,
  type IdeAgentApplyInput,
  type IdeAgentApplyReceipt,
  type IdeAgentAttachment,
  type IdeAgentBacklink,
  type IdeAgentCheckpoint,
  type IdeAgentCodeReceipt,
  type IdeAgentCodeSnapshot,
  type IdeAgentContextAssemblyInput,
  type IdeAgentDecision,
  type IdeAgentEvidenceFact,
  type IdeAgentPreimage,
  type IdeAgentProposal,
  type IdeAgentProposalInput,
  type IdeAgentProposalOperation,
  type IdeAgentRebaseInput,
  type IdeAgentReviewInput,
  type IdeAgentUndoInput,
} from "./agent-code-contract.ts"
import {
  IdeCheckpointRefSchema,
  IdeDiskRevisionRefSchema,
  IdeDocumentGenerationSchema,
  IdeDocumentRefSchema,
  IdeEvidenceRefSchema,
  IdeFileRefSchema,
  IdeProposalRefSchema,
  IdeTimestampSchema,
} from "./project-contract.ts"
import { DesktopWorkspacePathRefSchema } from "../workspace-contract.ts"

const boundedMessage = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(1_000))
const contentDigest = Schema.String.check(Schema.isPattern(/^sha256:[a-f0-9]{64}$/u))
const digestContent = (value: string): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`

const contentLineEnding = (value: string): "lf" | "crlf" | "mixed" | "none" => {
  const crlf = (value.match(/\r\n/gu) ?? []).length
  const lf = (value.match(/(?<!\r)\n/gu) ?? []).length
  if (crlf > 0 && lf > 0) return "mixed"
  if (crlf > 0) return "crlf"
  if (lf > 0) return "lf"
  return "none"
}

const proposalOperationViolation = (operation: IdeAgentProposalOperation): string | null => {
  if (operation._tag === "Create") {
    if (operation.base.existed || operation.base.content !== null || operation.base.diskRevisionRef !== null ||
      operation.base.documentRef !== null || operation.base.documentGeneration !== null || operation.base.contentDigest !== null ||
      operation.base.gitSnapshotRef !== null || operation.base.gitSnapshotGeneration !== null || operation.base.checkpointRef !== null ||
      operation.base.encoding !== "none" || operation.base.lineEnding !== "none" || operation.base.mode !== "none") {
      return "create must bind an exact missing base"
    }
    if (operation.policy.encoding === "preserve" || operation.policy.lineEnding === "preserve" || operation.policy.mode === "preserve") {
      return "create must state encoding, line ending, and mode explicitly"
    }
  } else {
    if (!operation.base.existed || operation.base.content === null || operation.base.diskRevisionRef === null ||
      operation.base.documentRef === null || operation.base.documentGeneration === null || operation.base.contentDigest === null ||
      operation.base.encoding === "none" || operation.base.lineEnding === "none" || operation.base.mode === "none") {
      return `${operation._tag.toLocaleLowerCase()} must bind a complete existing base`
    }
    if (operation.documentRef !== null && operation.documentRef !== operation.base.documentRef) {
      return `${operation._tag.toLocaleLowerCase()} document identity differs from its exact base`
    }
  }
  if (operation.policy.symlink !== "refuse") return "symlink target mutation is not admitted"
  if (operation._tag === "Create" || operation._tag === "Edit") {
    const content = operation._tag === "Create" ? operation.content : operation.targetContent
    const claimedDigest = operation._tag === "Create" ? operation.contentDigest : operation.targetContentDigest
    if (digestContent(content) !== claimedDigest) return "content digest does not match proposed bytes"
    const observedLineEnding = contentLineEnding(content)
    if (operation.policy.lineEnding !== "preserve" && observedLineEnding !== "none" && observedLineEnding !== operation.policy.lineEnding) {
      return `content uses ${observedLineEnding} while policy declares ${operation.policy.lineEnding}`
    }
  }
  return null
}

export const IdeAgentAuthorityFileSchema = Schema.Struct({
  pathRef: DesktopWorkspacePathRefSchema,
  fileRef: IdeFileRefSchema,
  documentRef: IdeDocumentRefSchema,
  documentGeneration: IdeDocumentGenerationSchema,
  diskRevisionRef: IdeDiskRevisionRefSchema,
  content: Schema.String.check(Schema.isMaxLength(1_000_000)),
  contentDigest,
  encoding: Schema.Literals(["utf-8", "utf-8-bom"]),
  lineEnding: Schema.Literals(["lf", "crlf", "mixed", "none"]),
  mode: Schema.Literals(["regular", "executable"]),
  dirty: Schema.Boolean,
  symlink: Schema.Boolean,
  contentClass: Schema.Literals(["text", "binary", "secret", "private", "too_large"]),
}).annotate({ identifier: "IdeAgentAuthorityFile" })
export type IdeAgentAuthorityFile = typeof IdeAgentAuthorityFileSchema.Type

export const IdeAgentAuthoritySnapshotSchema = Schema.TaggedUnion({
  Missing: { pathRef: DesktopWorkspacePathRefSchema },
  File: { file: IdeAgentAuthorityFileSchema },
  Unavailable: {
    pathRef: DesktopWorkspacePathRefSchema,
    reason: Schema.Literals([
      "grant_revoked", "permission_denied", "binary", "secret", "private", "too_large",
      "unsupported", "unavailable",
    ]),
    message: boundedMessage,
  },
}).annotate({ identifier: "IdeAgentAuthoritySnapshot" })
export type IdeAgentAuthoritySnapshot = typeof IdeAgentAuthoritySnapshotSchema.Type

export const IdeAgentAuthorityPostImageSchema = Schema.Struct({
  operationRef: IdeAgentOperationRefSchema,
  pathRef: DesktopWorkspacePathRefSchema,
  diskRevisionRef: Schema.NullOr(IdeDiskRevisionRefSchema),
  contentDigest: Schema.NullOr(contentDigest),
  documentRef: Schema.NullOr(IdeDocumentRefSchema),
  documentGeneration: Schema.NullOr(IdeDocumentGenerationSchema),
  encoding: Schema.NullOr(Schema.Literals(["utf-8", "utf-8-bom"])),
  lineEnding: Schema.NullOr(Schema.Literals(["lf", "crlf", "mixed", "none"])),
  mode: Schema.NullOr(Schema.Literals(["regular", "executable"])),
}).annotate({ identifier: "IdeAgentAuthorityPostImage" })
export type IdeAgentAuthorityPostImage = typeof IdeAgentAuthorityPostImageSchema.Type

export class IdeAgentAuthorityFailure extends Schema.TaggedErrorClass<IdeAgentAuthorityFailure>()(
  "IdeAgentCode.AuthorityFailure",
  {
    operation: Schema.String,
    reason: Schema.Literals(["conflict", "unsupported_policy", "grant_revoked", "unavailable", "rollback_failed"]),
    message: boundedMessage,
  },
) {}

export interface IdeAgentDocumentAuthorityShape {
  readonly snapshot: (
    pathRef: string,
  ) => Effect.Effect<IdeAgentAuthoritySnapshot, IdeAgentAuthorityFailure>
  readonly apply: (
    operation: IdeAgentProposalOperation,
  ) => Effect.Effect<IdeAgentAuthorityPostImage, IdeAgentAuthorityFailure>
  readonly restore: (
    operation: IdeAgentProposalOperation,
    preimage: IdeAgentPreimage,
    postImage: IdeAgentAuthorityPostImage,
  ) => Effect.Effect<IdeAgentAuthorityPostImage, IdeAgentAuthorityFailure>
}

export class IdeAgentDocumentAuthority extends Context.Service<
  IdeAgentDocumentAuthority,
  IdeAgentDocumentAuthorityShape
>()("@openagentsinc/openagents-desktop/IdeAgentDocumentAuthority") {}

export class IdeAgentCodeInvalidInput extends Schema.TaggedErrorClass<IdeAgentCodeInvalidInput>()(
  "IdeAgentCode.InvalidInput",
  { operation: Schema.String, detail: boundedMessage },
) {}

export class IdeAgentCodeStopped extends Schema.TaggedErrorClass<IdeAgentCodeStopped>()(
  "IdeAgentCode.Stopped",
  { operation: Schema.String, reason: boundedMessage },
) {}

export class IdeAgentCodeStaleGeneration extends Schema.TaggedErrorClass<IdeAgentCodeStaleGeneration>()(
  "IdeAgentCode.StaleGeneration",
  { operation: Schema.String, expected: Schema.Number, actual: Schema.Number },
) {}

export class IdeAgentCodeInvariantViolation extends Schema.TaggedErrorClass<IdeAgentCodeInvariantViolation>()(
  "IdeAgentCode.InvariantViolation",
  { operation: Schema.String, detail: boundedMessage },
) {}

export class IdeAgentCodeProposalState extends Schema.TaggedErrorClass<IdeAgentCodeProposalState>()(
  "IdeAgentCode.ProposalState",
  { operation: Schema.String, proposalRef: Schema.String, state: Schema.String, detail: boundedMessage },
) {}

export class IdeAgentCodeBaseChanged extends Schema.TaggedErrorClass<IdeAgentCodeBaseChanged>()(
  "IdeAgentCode.BaseChanged",
  {
    operation: Schema.String,
    proposalRef: Schema.String,
    operationRef: Schema.String,
    pathRef: Schema.String,
    reason: Schema.Literals([
      "created", "deleted", "revision_changed", "document_changed", "dirty_document", "symlink",
      "binary", "secret", "private", "too_large", "unsupported_policy", "unavailable",
    ]),
  },
) {}

export class IdeAgentCodeMissing extends Schema.TaggedErrorClass<IdeAgentCodeMissing>()(
  "IdeAgentCode.Missing",
  { operation: Schema.String, resource: Schema.String, ref: Schema.String },
) {}

export class IdeAgentCodeCheckpointExpired extends Schema.TaggedErrorClass<IdeAgentCodeCheckpointExpired>()(
  "IdeAgentCode.CheckpointExpired",
  { operation: Schema.String, checkpointRef: Schema.String, expiredAt: IdeTimestampSchema },
) {}

export const IdeAgentCodeServiceErrorSchema = Schema.Union([
  IdeAgentCodeInvalidInput,
  IdeAgentCodeStopped,
  IdeAgentCodeStaleGeneration,
  IdeAgentCodeInvariantViolation,
  IdeAgentCodeProposalState,
  IdeAgentCodeBaseChanged,
  IdeAgentCodeMissing,
  IdeAgentCodeCheckpointExpired,
  IdeAgentAuthorityFailure,
])
export type IdeAgentCodeServiceError = typeof IdeAgentCodeServiceErrorSchema.Type

export interface IdeAgentCodeServiceShape {
  readonly snapshot: () => Effect.Effect<IdeAgentCodeSnapshot, IdeAgentCodeStopped>
  readonly attach: (attachment: IdeAgentAttachment) => Effect.Effect<IdeAgentCodeSnapshot, IdeAgentCodeServiceError>
  readonly assembleManifest: (input: IdeAgentContextAssemblyInput) => Effect.Effect<IdeAgentCodeSnapshot, IdeAgentCodeServiceError>
  readonly submitProposal: (input: IdeAgentProposalInput) => Effect.Effect<IdeAgentCodeSnapshot, IdeAgentCodeServiceError>
  readonly beginReview: (input: IdeAgentReviewInput) => Effect.Effect<IdeAgentCodeSnapshot, IdeAgentCodeServiceError>
  readonly decide: (decision: IdeAgentDecision, expectedAttachmentGeneration: number) => Effect.Effect<IdeAgentCodeSnapshot, IdeAgentCodeServiceError>
  readonly apply: (input: IdeAgentApplyInput) => Effect.Effect<IdeAgentCodeSnapshot, IdeAgentCodeServiceError>
  readonly rebase: (input: IdeAgentRebaseInput) => Effect.Effect<IdeAgentCodeSnapshot, IdeAgentCodeServiceError>
  readonly undo: (input: IdeAgentUndoInput) => Effect.Effect<IdeAgentCodeSnapshot, IdeAgentCodeServiceError>
  readonly recordEvidence: (fact: IdeAgentEvidenceFact, expectedAttachmentGeneration: number) => Effect.Effect<IdeAgentCodeSnapshot, IdeAgentCodeServiceError>
  readonly receipt: () => Effect.Effect<IdeAgentCodeReceipt, IdeAgentCodeStopped>
  readonly stop: (reason: string) => Effect.Effect<IdeAgentCodeSnapshot, IdeAgentCodeStopped>
}

export class IdeAgentCodeService extends Context.Service<IdeAgentCodeService, IdeAgentCodeServiceShape>()(
  "@openagentsinc/openagents-desktop/IdeAgentCodeService",
) {}

const inputError = (operation: string, cause: unknown): IdeAgentCodeInvalidInput =>
  new IdeAgentCodeInvalidInput({ operation, detail: String(cause).slice(0, 1_000) || "invalid input" })

const decodeInput = <S extends Schema.ConstraintDecoder<unknown, never>>(
  operation: string,
  schema: S,
  value: unknown,
): Effect.Effect<S["Type"], IdeAgentCodeInvalidInput> =>
  Schema.decodeUnknownEffect(schema)(value).pipe(Effect.mapError(cause => inputError(operation, cause)))

const sameAttachment = (left: IdeAgentAttachment, right: IdeAgentAttachment): boolean =>
  left.agentAttachmentRef === right.agentAttachmentRef &&
  left.projectRef === right.projectRef &&
  left.rootRef === right.rootRef &&
  left.worktreeRef === right.worktreeRef &&
  left.sessionRef === right.sessionRef &&
  left.grantRef === right.grantRef &&
  left.attachmentGeneration === right.attachmentGeneration &&
  left.placementGeneration === right.placementGeneration

const operationRefs = (proposal: IdeAgentProposal): ReadonlyArray<string> =>
  proposal.operations.map(operation => operation.operationRef)

const duplicate = (values: ReadonlyArray<string>): string | null => {
  const seen = new Set<string>()
  for (const value of values) {
    if (seen.has(value)) return value
    seen.add(value)
  }
  return null
}

const assertAttachment = (
  operation: string,
  snapshot: IdeAgentCodeSnapshot,
  expectedGeneration: number,
): Effect.Effect<IdeAgentAttachment, IdeAgentCodeServiceError> => {
  if (snapshot.lifecycle === "stopped") {
    return Effect.fail(new IdeAgentCodeStopped({ operation, reason: "agent-code scope is stopped" }))
  }
  if (snapshot.attachment === null) {
    return Effect.fail(new IdeAgentCodeInvariantViolation({ operation, detail: "no project/worktree attachment is active" }))
  }
  if (snapshot.attachment.attachmentGeneration !== expectedGeneration) {
    return Effect.fail(new IdeAgentCodeStaleGeneration({
      operation,
      expected: expectedGeneration,
      actual: snapshot.attachment.attachmentGeneration,
    }))
  }
  return Effect.succeed(snapshot.attachment)
}

const proposalAt = (
  operation: string,
  snapshot: IdeAgentCodeSnapshot,
  proposalRef: string,
): Effect.Effect<IdeAgentProposal, IdeAgentCodeMissing> => {
  const proposal = snapshot.proposals.find(candidate => candidate.proposalRef === proposalRef)
  return proposal === undefined
    ? Effect.fail(new IdeAgentCodeMissing({ operation, resource: "proposal", ref: proposalRef }))
    : Effect.succeed(proposal)
}

const replaceProposal = (
  snapshot: IdeAgentCodeSnapshot,
  proposal: IdeAgentProposal,
): IdeAgentCodeSnapshot => ({
  ...snapshot,
  proposals: snapshot.proposals.map(candidate => candidate.proposalRef === proposal.proposalRef ? proposal : candidate),
  revision: snapshot.revision + 1,
})

const safeReceipt = (snapshot: IdeAgentCodeSnapshot): IdeAgentCodeReceipt => {
  const proposalCounts = {
    pending: 0,
    reviewing: 0,
    partial: 0,
    applied: 0,
    undone: 0,
    refused: 0,
    stale: 0,
  }
  for (const proposal of snapshot.proposals) {
    switch (proposal.lifecycle._tag) {
      case "Pending": proposalCounts.pending += 1; break
      case "Reviewing":
      case "Accepted":
      case "Applying": proposalCounts.reviewing += 1; break
      case "PartiallyAccepted": proposalCounts.partial += 1; break
      case "Applied": proposalCounts.applied += 1; break
      case "Undone": proposalCounts.undone += 1; break
      case "Rejected":
      case "Cancelled":
      case "Failed": proposalCounts.refused += 1; break
      case "RebaseRequired": proposalCounts.stale += 1; break
      case "Superseded": break
    }
  }
  const includedItemCount = snapshot.manifests.reduce(
    (total, manifest) => total + manifest.items.filter(item => item.disposition._tag === "Included").length,
    0,
  )
  const omittedItemCount = snapshot.manifests.reduce(
    (total, manifest) => total + manifest.items.filter(item => item.disposition._tag === "Omitted").length,
    0,
  )
  const passed = snapshot.evidence.filter(fact => fact.state._tag === "Passed").length
  const failed = snapshot.evidence.filter(fact => fact.state._tag === "Failed").length
  return IdeAgentCodeReceiptSchema.make({
    schemaVersion: "openagents.desktop.ide-agent-code.v1",
    lifecycle: snapshot.lifecycle,
    attachmentRef: snapshot.attachment?.agentAttachmentRef ?? null,
    projectRef: snapshot.attachment?.projectRef ?? null,
    worktreeRef: snapshot.attachment?.worktreeRef ?? null,
    attachmentGeneration: snapshot.attachment?.attachmentGeneration ?? null,
    manifestCount: snapshot.manifests.length,
    includedItemCount,
    omittedItemCount,
    proposalCounts,
    checkpointCount: snapshot.checkpoints.length,
    backlinkCount: snapshot.backlinks.length,
    evidenceCounts: { observed: snapshot.evidence.length, passed, failed },
    containsPrivateContent: false,
  })
}

const preimageFor = (
  operation: IdeAgentProposalOperation,
  snapshot: IdeAgentAuthoritySnapshot,
): IdeAgentPreimage => snapshot._tag === "Missing"
  ? IdeAgentPreimageSchema.cases.Missing.make({ operationRef: operation.operationRef, pathRef: operation.pathRef })
  : snapshot._tag === "File"
    ? IdeAgentPreimageSchema.cases.File.make({
        operationRef: operation.operationRef,
        pathRef: operation.pathRef,
        content: snapshot.file.content,
        contentDigest: snapshot.file.contentDigest,
        diskRevisionRef: snapshot.file.diskRevisionRef,
        encoding: snapshot.file.encoding,
        lineEnding: snapshot.file.lineEnding,
        mode: snapshot.file.mode,
      })
    : IdeAgentPreimageSchema.cases.Missing.make({ operationRef: operation.operationRef, pathRef: operation.pathRef })

const validateBase = (
  proposal: IdeAgentProposal,
  operation: IdeAgentProposalOperation,
  current: IdeAgentAuthoritySnapshot,
): Effect.Effect<void, IdeAgentCodeBaseChanged | IdeAgentAuthorityFailure> => {
  if (current._tag === "Unavailable") {
    switch (current.reason) {
      case "binary":
      case "secret":
      case "private":
      case "too_large":
      case "unsupported":
        return Effect.fail(new IdeAgentCodeBaseChanged({
          operation: "IdeAgentCode.apply.preflight",
          proposalRef: proposal.proposalRef,
          operationRef: operation.operationRef,
          pathRef: operation.pathRef,
          reason: current.reason === "unsupported" ? "unsupported_policy" : current.reason,
        }))
      case "grant_revoked":
      case "permission_denied":
      case "unavailable":
        break
    }
    return Effect.fail(new IdeAgentAuthorityFailure({
      operation: "IdeAgentCode.apply.preflight",
      reason: current.reason === "grant_revoked" ? "grant_revoked" : "unavailable",
      message: current.message,
    }))
  }
  if (operation._tag === "Create") {
    return current._tag === "Missing"
      ? Effect.void
      : Effect.fail(new IdeAgentCodeBaseChanged({
          operation: "IdeAgentCode.apply.preflight",
          proposalRef: proposal.proposalRef,
          operationRef: operation.operationRef,
          pathRef: operation.pathRef,
          reason: "created",
        }))
  }
  if (current._tag === "Missing") {
    return Effect.fail(new IdeAgentCodeBaseChanged({
      operation: "IdeAgentCode.apply.preflight",
      proposalRef: proposal.proposalRef,
      operationRef: operation.operationRef,
      pathRef: operation.pathRef,
      reason: "deleted",
    }))
  }
  const file = current.file
  const base = operation.base
  const changed = (reason: IdeAgentCodeBaseChanged["reason"]) => Effect.fail(new IdeAgentCodeBaseChanged({
    operation: "IdeAgentCode.apply.preflight",
    proposalRef: proposal.proposalRef,
    operationRef: operation.operationRef,
    pathRef: operation.pathRef,
    reason,
  }))
  if (file.dirty) return changed("dirty_document")
  if (file.symlink || operation.policy.symlink !== "refuse") return changed("symlink")
  if (file.contentClass !== "text") return changed(file.contentClass)
  if (base.diskRevisionRef === null || base.diskRevisionRef !== file.diskRevisionRef) return changed("revision_changed")
  if (base.contentDigest === null || base.contentDigest !== file.contentDigest) return changed("revision_changed")
  if (base.documentRef !== null && base.documentRef !== file.documentRef) return changed("document_changed")
  if (base.documentGeneration !== null && base.documentGeneration !== file.documentGeneration) return changed("document_changed")
  if (operation.policy.encoding !== "preserve" && operation.policy.encoding !== file.encoding) return changed("unsupported_policy")
  if (operation.policy.lineEnding !== "preserve" && file.lineEnding === "mixed") return changed("unsupported_policy")
  return Effect.void
}

const timestampPlus = (timestamp: string, milliseconds: number): string =>
  new Date(Date.parse(timestamp) + milliseconds).toISOString()

export const makeIdeAgentCodeLayer = (
  seed: unknown = emptyIdeAgentCodeSnapshot(),
  options: Readonly<{
    now?: () => typeof IdeTimestampSchema.Type
    checkpointRetentionMs?: number
  }> = {},
): Layer.Layer<IdeAgentCodeService, IdeAgentCodeInvalidInput, IdeAgentDocumentAuthority> =>
  Layer.effect(IdeAgentCodeService, Effect.gen(function* () {
    const now = options.now ?? (() => IdeTimestampSchema.make(new Date().toISOString()))
    const decodedInitial = yield* decodeInput("IdeAgentCode.acquire", IdeAgentCodeSnapshotSchema, seed)
    const acquiredAt = now()
    const expiredCheckpointRefs = new Set(decodedInitial.checkpoints
      .filter(checkpoint => Date.parse(checkpoint.expiresAt) < Date.parse(acquiredAt))
      .map(checkpoint => checkpoint.checkpointRef))
    const initial = expiredCheckpointRefs.size === 0 ? decodedInitial : IdeAgentCodeSnapshotSchema.make({
      ...decodedInitial,
      checkpoints: decodedInitial.checkpoints.filter(checkpoint => !expiredCheckpointRefs.has(checkpoint.checkpointRef)),
      backlinks: decodedInitial.backlinks.map(backlink => backlink.resolution._tag !== "Historical" ||
        !expiredCheckpointRefs.has(backlink.resolution.checkpointRef) ? backlink : IdeAgentBacklinkSchema.make({
          ...backlink,
          resolution: { _tag: "Unavailable", reason: "retention_expired" },
        })),
      revision: decodedInitial.revision + 1,
    })
    const authority = yield* IdeAgentDocumentAuthority
    const state = yield* Ref.make(initial)
    const stopped = yield* Ref.make<string | null>(initial.lifecycle === "stopped" ? "restored stopped state" : null)
    const ordinal = yield* Ref.make(0)
    const lock = yield* Semaphore.make(1)
    const checkpointRetentionMs = options.checkpointRetentionMs ?? 24 * 60 * 60 * 1_000

    const nextRef = Effect.fn("IdeAgentCode.nextRef")(function* (prefix: string) {
      const value = yield* Ref.updateAndGet(ordinal, current => current + 1)
      return `${prefix}${value}`
    })

    const ensureActive = Effect.fn("IdeAgentCode.ensureActive")(function* (operation: string) {
      const reason = yield* Ref.get(stopped)
      if (reason !== null) return yield* Effect.fail(new IdeAgentCodeStopped({ operation, reason }))
    })

    const snapshot = Effect.fn("IdeAgentCode.snapshot")(function* () {
      yield* ensureActive("IdeAgentCode.snapshot")
      return yield* Ref.get(state)
    })

    const attach = Effect.fn("IdeAgentCode.attach")(function* (raw: IdeAgentAttachment) {
      yield* ensureActive("IdeAgentCode.attach")
      const attachment = yield* decodeInput("IdeAgentCode.attach", IdeAgentAttachmentSchema, raw)
      return yield* lock.withPermit(Effect.gen(function* () {
        const current = yield* Ref.get(state)
        const active = current.attachment
        if (active !== null && attachment.attachmentGeneration < active.attachmentGeneration) {
          return yield* Effect.fail(new IdeAgentCodeStaleGeneration({
            operation: "IdeAgentCode.attach",
            expected: attachment.attachmentGeneration,
            actual: active.attachmentGeneration,
          }))
        }
        if (active !== null && attachment.attachmentGeneration === active.attachmentGeneration && !sameAttachment(active, attachment)) {
          return yield* Effect.fail(new IdeAgentCodeInvariantViolation({
            operation: "IdeAgentCode.attach",
            detail: "one attachment generation cannot be rebound to another project, worktree, session, grant, or placement",
          }))
        }
        if (active !== null && sameAttachment(active, attachment)) return current
        const proposals = current.proposals.map(proposal => {
          if (["Applied", "Undone", "Rejected", "Cancelled", "Superseded", "Failed"].includes(proposal.lifecycle._tag)) return proposal
          return IdeAgentProposalSchema.make({
            ...proposal,
            lifecycle: { _tag: "Cancelled", reason: "attachment generation replaced before proposal settlement" },
          })
        })
        const next = IdeAgentCodeSnapshotSchema.make({
          ...current,
          attachment,
          manifests: [],
          proposals,
          lifecycle: "attached",
          revision: current.revision + 1,
        })
        yield* Ref.set(state, next)
        return next
      }))
    })

    const assembleManifest = Effect.fn("IdeAgentCode.assembleManifest")(function* (raw: IdeAgentContextAssemblyInput) {
      yield* ensureActive("IdeAgentCode.assembleManifest")
      const input = yield* decodeInput("IdeAgentCode.assembleManifest", IdeAgentContextAssemblyInputSchema, raw)
      return yield* lock.withPermit(Ref.modify(state, current => {
        const attachmentEffect = assertAttachment("IdeAgentCode.assembleManifest", current, input.expectedAttachmentGeneration)
        const included = input.manifest.items.filter(item => item.disposition._tag === "Included")
        const omitted = input.manifest.items.filter(item => item.disposition._tag === "Omitted")
        const calculatedBytes = included.reduce((total, item) => total + item.byteEstimate, 0)
        const calculatedTokens = included.reduce((total, item) => total + item.tokenEstimate, 0)
        const duplicateRef = duplicate(input.manifest.items.map(item => item.contextItemRef))
        const validate = Effect.gen(function* () {
          const attachment = yield* attachmentEffect
          if (!sameAttachment(attachment, input.manifest.attachment)) {
            return yield* Effect.fail(new IdeAgentCodeInvariantViolation({
              operation: "IdeAgentCode.assembleManifest",
              detail: "manifest attachment does not match the active exact project/worktree/session generation",
            }))
          }
          if (duplicateRef !== null) {
            return yield* Effect.fail(new IdeAgentCodeInvariantViolation({
              operation: "IdeAgentCode.assembleManifest",
              detail: `duplicate context item ref: ${duplicateRef}`,
            }))
          }
          if (calculatedBytes !== input.manifest.includedBytes || calculatedTokens !== input.manifest.includedTokens || omitted.length !== input.manifest.omittedCount) {
            return yield* Effect.fail(new IdeAgentCodeInvariantViolation({
              operation: "IdeAgentCode.assembleManifest",
              detail: "manifest totals do not account exactly for included bytes/tokens and omitted items",
            }))
          }
          if (calculatedBytes > input.manifest.byteBudget || calculatedTokens > input.manifest.tokenBudget) {
            return yield* Effect.fail(new IdeAgentCodeInvariantViolation({
              operation: "IdeAgentCode.assembleManifest",
              detail: "included context exceeds its disclosed byte or token budget",
            }))
          }
          const existingManifest = current.manifests.find(manifest => manifest.manifestRef === input.manifest.manifestRef)
          if (existingManifest !== undefined) {
            if (JSON.stringify(existingManifest) === JSON.stringify(input.manifest)) return current
            return yield* Effect.fail(new IdeAgentCodeInvariantViolation({
              operation: "IdeAgentCode.assembleManifest",
              detail: "manifest ref was reused with different context bytes or policy",
            }))
          }
          const manifests = [
            ...current.manifests,
            input.manifest,
          ].slice(-100)
          return IdeAgentCodeSnapshotSchema.make({ ...current, manifests, revision: current.revision + 1 })
        })
        return [validate, current] as const
      }).pipe(Effect.flatten).pipe(Effect.tap(next => Ref.set(state, next))))
    })

    const submitProposal = Effect.fn("IdeAgentCode.submitProposal")(function* (raw: IdeAgentProposalInput) {
      yield* ensureActive("IdeAgentCode.submitProposal")
      const input = yield* decodeInput("IdeAgentCode.submitProposal", IdeAgentProposalInputSchema, raw)
      return yield* lock.withPermit(Ref.modify(state, current => {
        const validate = Effect.gen(function* () {
          const attachment = yield* assertAttachment("IdeAgentCode.submitProposal", current, input.expectedAttachmentGeneration)
          if (!sameAttachment(attachment, input.proposal.attachment)) {
            return yield* Effect.fail(new IdeAgentCodeInvariantViolation({
              operation: "IdeAgentCode.submitProposal",
              detail: "proposal attachment does not match the active project/worktree/session generation",
            }))
          }
          const proposalManifest = current.manifests.find(manifest =>
            manifest.manifestRef === input.proposal.manifestRef && sameAttachment(manifest.attachment, attachment))
          if (proposalManifest === undefined) {
            return yield* Effect.fail(new IdeAgentCodeMissing({
              operation: "IdeAgentCode.submitProposal",
              resource: "manifest",
              ref: input.proposal.manifestRef,
            }))
          }
          if (proposalManifest.turnRef !== input.proposal.turnRef ||
            proposalManifest.conversationThreadRef !== input.proposal.conversationThreadRef) {
            return yield* Effect.fail(new IdeAgentCodeInvariantViolation({
              operation: "IdeAgentCode.submitProposal",
              detail: "proposal turn/conversation backlink differs from its exact context manifest",
            }))
          }
          const existingProposal = current.proposals.find(proposal => proposal.proposalRef === input.proposal.proposalRef)
          if (existingProposal !== undefined) {
            if (JSON.stringify(existingProposal) === JSON.stringify(input.proposal)) return current
            return yield* Effect.fail(new IdeAgentCodeInvariantViolation({
              operation: "IdeAgentCode.submitProposal",
              detail: "proposal ref was reused with different content or identity",
            }))
          }
          const duplicateOperation = duplicate(operationRefs(input.proposal))
          if (duplicateOperation !== null) {
            return yield* Effect.fail(new IdeAgentCodeInvariantViolation({
              operation: "IdeAgentCode.submitProposal",
              detail: `duplicate proposal operation ref: ${duplicateOperation}`,
            }))
          }
          if (input.proposal.lifecycle._tag !== "Pending") {
            return yield* Effect.fail(new IdeAgentCodeProposalState({
              operation: "IdeAgentCode.submitProposal",
              proposalRef: input.proposal.proposalRef,
              state: input.proposal.lifecycle._tag,
              detail: "untrusted runtime proposals must enter in Pending state",
            }))
          }
          const invalidOperation = input.proposal.operations
            .map(operation => ({ operation, violation: proposalOperationViolation(operation) }))
            .find(candidate => candidate.violation !== null)
          if (invalidOperation !== undefined) {
            return yield* Effect.fail(new IdeAgentCodeInvariantViolation({
              operation: "IdeAgentCode.submitProposal",
              detail: `proposal operation ${invalidOperation.operation.operationRef}: ${invalidOperation.violation}`,
            }))
          }
          const pathTargets = input.proposal.operations.flatMap(operation =>
            operation._tag === "Rename" ? [operation.pathRef, operation.targetPathRef] : [operation.pathRef])
          const duplicatePath = duplicate(pathTargets)
          if (duplicatePath !== null) {
            return yield* Effect.fail(new IdeAgentCodeInvariantViolation({
              operation: "IdeAgentCode.submitProposal",
              detail: `one proposal cannot ambiguously mutate the same path twice: ${duplicatePath}`,
            }))
          }
          return IdeAgentCodeSnapshotSchema.make({
            ...current,
            proposals: [...current.proposals, input.proposal].slice(-200),
            revision: current.revision + 1,
          })
        })
        return [validate, current] as const
      }).pipe(Effect.flatten).pipe(Effect.tap(next => Ref.set(state, next))))
    })

    const beginReview = Effect.fn("IdeAgentCode.beginReview")(function* (raw: IdeAgentReviewInput) {
      yield* ensureActive("IdeAgentCode.beginReview")
      const input = yield* decodeInput("IdeAgentCode.beginReview", IdeAgentReviewInputSchema, raw)
      return yield* lock.withPermit(Effect.gen(function* () {
        const current = yield* Ref.get(state)
        yield* assertAttachment("IdeAgentCode.beginReview", current, input.expectedAttachmentGeneration)
        const proposal = yield* proposalAt("IdeAgentCode.beginReview", current, input.proposalRef)
        if (proposal.lifecycle._tag === "Reviewing") {
          if (proposal.lifecycle.reviewRef === input.reviewRef) return current
          return yield* Effect.fail(new IdeAgentCodeInvariantViolation({
            operation: "IdeAgentCode.beginReview",
            detail: "review ref changed during an active exact proposal review",
          }))
        }
        if (proposal.lifecycle._tag !== "Pending") {
          return yield* Effect.fail(new IdeAgentCodeProposalState({
            operation: "IdeAgentCode.beginReview",
            proposalRef: proposal.proposalRef,
            state: proposal.lifecycle._tag,
            detail: "only pending proposals can enter review",
          }))
        }
        const nextProposal = IdeAgentProposalSchema.make({ ...proposal, lifecycle: { _tag: "Reviewing", reviewRef: input.reviewRef } })
        const next = replaceProposal(current, nextProposal)
        yield* Ref.set(state, next)
        return next
      }))
    })

    const decide = Effect.fn("IdeAgentCode.decide")(function* (
      raw: IdeAgentDecision,
      expectedAttachmentGeneration: number,
    ) {
      yield* ensureActive("IdeAgentCode.decide")
      const decision = yield* decodeInput("IdeAgentCode.decide", IdeAgentDecisionSchema, raw)
      return yield* lock.withPermit(Effect.gen(function* () {
        const current = yield* Ref.get(state)
        yield* assertAttachment("IdeAgentCode.decide", current, expectedAttachmentGeneration)
        const existingDecision = current.decisions.find(candidate => candidate.decisionRef === decision.decisionRef)
        if (existingDecision !== undefined) {
          if (JSON.stringify(existingDecision) === JSON.stringify(decision)) return current
          return yield* Effect.fail(new IdeAgentCodeInvariantViolation({
            operation: "IdeAgentCode.decide",
            detail: "decision ref was reused with different proposal or disposition",
          }))
        }
        const proposal = yield* proposalAt("IdeAgentCode.decide", current, decision.proposalRef)
        if (proposal.lifecycle._tag !== "Pending" && proposal.lifecycle._tag !== "Reviewing") {
          return yield* Effect.fail(new IdeAgentCodeProposalState({
            operation: "IdeAgentCode.decide",
            proposalRef: proposal.proposalRef,
            state: proposal.lifecycle._tag,
            detail: "proposal is no longer decision-eligible",
          }))
        }
        const all = new Set(operationRefs(proposal))
        const selected = [...new Set(decision.operationRefs)]
        if (selected.some(ref => !all.has(ref))) {
          return yield* Effect.fail(new IdeAgentCodeInvariantViolation({
            operation: "IdeAgentCode.decide",
            detail: "decision references an operation outside the exact proposal",
          }))
        }
        const remaining = proposal.operations.filter(operation => !selected.includes(operation.operationRef))
        const decisions = [...current.decisions, decision].slice(-500)
        if (decision.disposition === "reject" && remaining.length === 0) {
          const next = replaceProposal({ ...current, decisions }, IdeAgentProposalSchema.make({
            ...proposal,
            lifecycle: { _tag: "Rejected", decisionRef: decision.decisionRef, reason: decision.reason ?? "owner rejected proposal" },
          }))
          yield* Ref.set(state, next)
          return next
        }
        if (decision.disposition === "accept" && remaining.length === 0) {
          const next = replaceProposal({ ...current, decisions }, IdeAgentProposalSchema.make({
            ...proposal,
            lifecycle: { _tag: "Accepted", acceptedOperationRefs: selected },
          }))
          yield* Ref.set(state, next)
          return next
        }
        const acceptedOperations = decision.disposition === "accept"
          ? proposal.operations.filter(operation => selected.includes(operation.operationRef))
          : remaining
        const rejectedRefs = decision.disposition === "reject" ? selected : remaining.map(operation => operation.operationRef)
        const childProposalRef = IdeProposalRefSchema.make(yield* nextRef("ide.proposal.partial."))
        const child = IdeAgentProposalSchema.make({
          ...proposal,
          proposalRef: childProposalRef,
          parentProposalRef: proposal.proposalRef,
          operations: acceptedOperations,
          createdAt: now(),
          lifecycle: decision.disposition === "accept"
            ? { _tag: "Accepted", acceptedOperationRefs: acceptedOperations.map(operation => operation.operationRef) }
            : { _tag: "Pending" },
        })
        const parent = IdeAgentProposalSchema.make({
          ...proposal,
          lifecycle: {
            _tag: "PartiallyAccepted",
            acceptedOperationRefs: acceptedOperations.map(operation => operation.operationRef),
            rejectedOperationRefs: rejectedRefs,
            childProposalRef,
          },
        })
        const next = IdeAgentCodeSnapshotSchema.make({
          ...current,
          decisions,
          proposals: [...current.proposals.map(candidate => candidate.proposalRef === parent.proposalRef ? parent : candidate), child].slice(-200),
          revision: current.revision + 1,
        })
        yield* Ref.set(state, next)
        return next
      }))
    })

    const apply = Effect.fn("IdeAgentCode.apply")(function* (raw: IdeAgentApplyInput) {
      yield* ensureActive("IdeAgentCode.apply")
      const input = yield* decodeInput("IdeAgentCode.apply", IdeAgentApplyInputSchema, raw)
      return yield* lock.withPermit(Effect.gen(function* () {
        let current = yield* Ref.get(state)
        const attachment = yield* assertAttachment("IdeAgentCode.apply", current, input.expectedAttachmentGeneration)
        if (current.revision !== input.expectedProposalRevision) {
          return yield* Effect.fail(new IdeAgentCodeInvariantViolation({
            operation: "IdeAgentCode.apply",
            detail: `proposal graph revision changed (${input.expectedProposalRevision} -> ${current.revision})`,
          }))
        }
        const proposal = yield* proposalAt("IdeAgentCode.apply", current, input.proposalRef)
        if (proposal.lifecycle._tag !== "Accepted") {
          return yield* Effect.fail(new IdeAgentCodeProposalState({
            operation: "IdeAgentCode.apply",
            proposalRef: proposal.proposalRef,
            state: proposal.lifecycle._tag,
            detail: "only an explicitly accepted proposal can apply",
          }))
        }
        if (!sameAttachment(attachment, proposal.attachment)) {
          return yield* Effect.fail(new IdeAgentCodeStaleGeneration({
            operation: "IdeAgentCode.apply",
            expected: proposal.attachment.attachmentGeneration,
            actual: attachment.attachmentGeneration,
          }))
        }
        const requested = [...new Set(input.operationRefs)]
        const accepted = proposal.lifecycle.acceptedOperationRefs
        if (requested.length !== accepted.length || requested.some(ref => !accepted.includes(ref))) {
          return yield* Effect.fail(new IdeAgentCodeInvariantViolation({
            operation: "IdeAgentCode.apply",
            detail: "apply granularity must equal the exact accepted operation set; partial decisions create a child proposal",
          }))
        }
        const operations = proposal.operations.filter(operation => requested.includes(operation.operationRef))
        const observations: Array<Readonly<{
          operationRef: string
          pathRef: string
          snapshot: IdeAgentAuthoritySnapshot
        }>> = []
        const preflight = yield* Effect.gen(function* () {
          const snapshots: IdeAgentAuthoritySnapshot[] = []
          for (const operation of operations) {
            const observed = yield* authority.snapshot(operation.pathRef)
            observations.push({ operationRef: operation.operationRef, pathRef: operation.pathRef, snapshot: observed })
            yield* validateBase(proposal, operation, observed)
            if (operation._tag === "Rename") {
              const target = yield* authority.snapshot(operation.targetPathRef)
              observations.push({ operationRef: operation.operationRef, pathRef: operation.targetPathRef, snapshot: target })
              if (target._tag !== "Missing") {
                return yield* Effect.fail(new IdeAgentCodeBaseChanged({
                  operation: "IdeAgentCode.apply.preflight",
                  proposalRef: proposal.proposalRef,
                  operationRef: operation.operationRef,
                  pathRef: operation.targetPathRef,
                  reason: "created",
                }))
              }
            }
            snapshots.push(observed)
          }
          return snapshots
        }).pipe(Effect.match({
          onFailure: error => ({ ok: false as const, error }),
          onSuccess: snapshots => ({ ok: true as const, snapshots }),
        }))
        if (!preflight.ok) {
          if (preflight.error instanceof IdeAgentCodeBaseChanged) {
            const error = preflight.error
            const currentObservation = observations.find(observation =>
              observation.operationRef === error.operationRef && observation.pathRef === error.pathRef)
            const currentSnapshot = currentObservation?.snapshot
            const rebasing = replaceProposal(current, IdeAgentProposalSchema.make({
              ...proposal,
              lifecycle: {
                _tag: "RebaseRequired",
                reason: error.reason,
                conflictCount: 1,
                currentPathRef: error.pathRef,
                currentState: currentSnapshot?._tag === "File" ? "file" : currentSnapshot?._tag === "Missing" ? "missing" : "unavailable",
                currentDiskRevisionRef: currentSnapshot?._tag === "File" ? currentSnapshot.file.diskRevisionRef : null,
                currentDocumentGeneration: currentSnapshot?._tag === "File" ? currentSnapshot.file.documentGeneration : null,
                currentContentDigest: currentSnapshot?._tag === "File" ? currentSnapshot.file.contentDigest : null,
              },
            }))
            yield* Ref.set(state, rebasing)
          }
          return yield* Effect.fail(preflight.error)
        }
        const snapshots = preflight.snapshots
        const checkpointRef = IdeCheckpointRefSchema.make(yield* nextRef("ide.checkpoint.agent."))
        const applyRef = IdeAgentApplyRefSchema.make(yield* nextRef("ide.agent-apply."))
        const createdAt = now()
        const checkpoint = IdeAgentCheckpointSchema.make({
          checkpointRef,
          proposalRef: proposal.proposalRef,
          attachment,
          createdAt,
          expiresAt: IdeTimestampSchema.make(timestampPlus(createdAt, checkpointRetentionMs)),
          preimages: operations.map((operation, index) => preimageFor(operation, snapshots[index]!)),
          consumedByUndoRef: null,
        })
        current = IdeAgentCodeSnapshotSchema.make({
          ...replaceProposal(current, IdeAgentProposalSchema.make({
            ...proposal,
            lifecycle: { _tag: "Applying", applyRef, checkpointRef },
          })),
          checkpoints: [...current.checkpoints, checkpoint].slice(-200),
        })
        yield* Ref.set(state, current)
        const applied: Array<Readonly<{
          operation: IdeAgentProposalOperation
          preimage: IdeAgentPreimage
          postImage: IdeAgentAuthorityPostImage
        }>> = []
        const attempted = yield* Effect.forEach(operations, (operation, index) =>
          authority.apply(operation).pipe(Effect.tap(postImage => Effect.sync(() => {
            applied.push({ operation, preimage: checkpoint.preimages[index]!, postImage })
          }))), { concurrency: 1 }).pipe(Effect.exit)
        if (attempted._tag === "Failure") {
          let rollbackFailed = false
          for (const entry of [...applied].reverse()) {
            const restored = yield* authority.restore(entry.operation, entry.preimage, entry.postImage).pipe(Effect.exit)
            if (restored._tag === "Failure") rollbackFailed = true
          }
          const failedProposal = IdeAgentProposalSchema.make({
            ...proposal,
            lifecycle: rollbackFailed
              ? { _tag: "Failed", reason: "apply failed and compensating rollback did not complete", recoverable: false }
              : {
                  _tag: "RebaseRequired",
                  reason: "base_changed",
                  conflictCount: 1,
                  currentPathRef: operations[0]!.pathRef,
                  currentState: "unavailable",
                  currentDiskRevisionRef: null,
                  currentDocumentGeneration: null,
                  currentContentDigest: null,
                },
          })
          const failed = replaceProposal(yield* Ref.get(state), failedProposal)
          yield* Ref.set(state, failed)
          if (rollbackFailed) {
            return yield* Effect.fail(new IdeAgentAuthorityFailure({
              operation: "IdeAgentCode.apply.rollback",
              reason: "rollback_failed",
              message: "The canonical authority could not completely restore the preimage after an apply failure.",
            }))
          }
          return yield* Effect.fail(new IdeAgentAuthorityFailure({
            operation: "IdeAgentCode.apply",
            reason: "conflict",
            message: "The canonical authority refused an operation; all completed operations were restored.",
          }))
        }
        const postImages = attempted.value
        const undoableUntil = checkpoint.expiresAt
        const receipt = IdeAgentApplyReceiptSchema.make({
          applyRef,
          proposalRef: proposal.proposalRef,
          checkpointRef,
          attachment,
          appliedAt: now(),
          operationRefs: requested,
          postImageRevisionRefs: postImages.map(postImage => ({
            operationRef: postImage.operationRef,
            pathRef: postImage.pathRef,
            diskRevisionRef: postImage.diskRevisionRef,
            contentDigest: postImage.contentDigest,
            encoding: postImage.encoding,
            lineEnding: postImage.lineEnding,
            mode: postImage.mode,
          })),
          rollback: "not_needed",
          undoableUntil,
        })
        const backlinks: IdeAgentBacklink[] = operations.flatMap((operation, index) => {
          const postImage = postImages[index]!
          if (postImage.documentRef === null || postImage.documentGeneration === null) return []
          return [IdeAgentBacklinkSchema.make({
            backlinkRef: IdeAgentBacklinkRefSchema.make(`ide.agent-backlink.${applyRef.split(".").at(-1)}.${index + 1}`),
            proposalRef: proposal.proposalRef,
            operationRef: operation.operationRef,
            sessionRef: proposal.sessionRef,
            turnRef: proposal.turnRef,
            conversationThreadRef: proposal.conversationThreadRef,
            attachmentGeneration: attachment.attachmentGeneration,
            createdAt: now(),
            resolution: {
              _tag: "Current",
              fileRef: operation.fileRef,
              documentRef: postImage.documentRef,
              documentGeneration: postImage.documentGeneration,
              pathRef: postImage.pathRef,
              range: null,
            },
          })]
        })
        const next = IdeAgentCodeSnapshotSchema.make({
          ...replaceProposal(yield* Ref.get(state), IdeAgentProposalSchema.make({
            ...proposal,
            lifecycle: { _tag: "Applied", applyRef, checkpointRef, undoableUntil },
          })),
          applyReceipts: [...(yield* Ref.get(state)).applyReceipts, receipt].slice(-200),
          backlinks: [...(yield* Ref.get(state)).backlinks, ...backlinks].slice(-1_000),
        })
        yield* Ref.set(state, next)
        return next
      }))
    })

    const rebase = Effect.fn("IdeAgentCode.rebase")(function* (raw: IdeAgentRebaseInput) {
      yield* ensureActive("IdeAgentCode.rebase")
      const input = yield* decodeInput("IdeAgentCode.rebase", IdeAgentRebaseInputSchema, raw)
      return yield* lock.withPermit(Effect.gen(function* () {
        const current = yield* Ref.get(state)
        const attachment = yield* assertAttachment("IdeAgentCode.rebase", current, input.expectedAttachmentGeneration)
        const original = yield* proposalAt("IdeAgentCode.rebase", current, input.proposalRef)
        if (original.lifecycle._tag !== "RebaseRequired") {
          return yield* Effect.fail(new IdeAgentCodeProposalState({
            operation: "IdeAgentCode.rebase",
            proposalRef: original.proposalRef,
            state: original.lifecycle._tag,
            detail: "an explicit replacement is accepted only from RebaseRequired",
          }))
        }
        const replacement = input.replacementProposal
        if (
          replacement.parentProposalRef !== original.proposalRef ||
          replacement.proposalRef === original.proposalRef ||
          replacement.manifestRef !== original.manifestRef ||
          replacement.turnRef !== original.turnRef ||
          replacement.lifecycle._tag !== "Pending" ||
          !sameAttachment(attachment, replacement.attachment)
        ) {
          return yield* Effect.fail(new IdeAgentCodeInvariantViolation({
            operation: "IdeAgentCode.rebase",
            detail: "replacement must be a new pending child on the same manifest, turn, and exact attachment",
          }))
        }
        const superseded = IdeAgentProposalSchema.make({
          ...original,
          lifecycle: { _tag: "Superseded", replacementProposalRef: replacement.proposalRef },
        })
        const next = IdeAgentCodeSnapshotSchema.make({
          ...current,
          proposals: [...current.proposals.map(proposal => proposal.proposalRef === original.proposalRef ? superseded : proposal), replacement].slice(-200),
          revision: current.revision + 1,
        })
        yield* Ref.set(state, next)
        return next
      }))
    })

    const undo = Effect.fn("IdeAgentCode.undo")(function* (raw: IdeAgentUndoInput) {
      yield* ensureActive("IdeAgentCode.undo")
      const input = yield* decodeInput("IdeAgentCode.undo", IdeAgentUndoInputSchema, raw)
      return yield* lock.withPermit(Effect.gen(function* () {
        const current = yield* Ref.get(state)
        yield* assertAttachment("IdeAgentCode.undo", current, input.expectedAttachmentGeneration)
        const proposal = yield* proposalAt("IdeAgentCode.undo", current, input.proposalRef)
        if (proposal.lifecycle._tag !== "Applied" || proposal.lifecycle.applyRef !== input.applyRef || proposal.lifecycle.checkpointRef !== input.checkpointRef) {
          return yield* Effect.fail(new IdeAgentCodeProposalState({
            operation: "IdeAgentCode.undo",
            proposalRef: proposal.proposalRef,
            state: proposal.lifecycle._tag,
            detail: "undo must bind the exact applied proposal, apply receipt, and checkpoint",
          }))
        }
        const checkpoint = current.checkpoints.find(candidate => candidate.checkpointRef === input.checkpointRef)
        const receipt = current.applyReceipts.find(candidate => candidate.applyRef === input.applyRef)
        if (checkpoint === undefined || receipt === undefined) {
          if (checkpoint === undefined && Date.parse(now()) > Date.parse(proposal.lifecycle.undoableUntil)) {
            return yield* Effect.fail(new IdeAgentCodeCheckpointExpired({
              operation: "IdeAgentCode.undo",
              checkpointRef: input.checkpointRef,
              expiredAt: proposal.lifecycle.undoableUntil,
            }))
          }
          return yield* Effect.fail(new IdeAgentCodeMissing({
            operation: "IdeAgentCode.undo",
            resource: checkpoint === undefined ? "checkpoint" : "apply_receipt",
            ref: checkpoint === undefined ? input.checkpointRef : input.applyRef,
          }))
        }
        if (checkpoint.consumedByUndoRef !== null) {
          return yield* Effect.fail(new IdeAgentCodeProposalState({
            operation: "IdeAgentCode.undo",
            proposalRef: proposal.proposalRef,
            state: "undo_consumed",
            detail: "checkpoint was already consumed by an earlier undo",
          }))
        }
        const observedNow = now()
        if (Date.parse(observedNow) > Date.parse(checkpoint.expiresAt)) {
          return yield* Effect.fail(new IdeAgentCodeCheckpointExpired({
            operation: "IdeAgentCode.undo",
            checkpointRef: checkpoint.checkpointRef,
            expiredAt: checkpoint.expiresAt,
          }))
        }
        const operations = proposal.operations.filter(operation => receipt.operationRefs.includes(operation.operationRef))
        const restoredRefs: Array<typeof IdeAgentOperationRefSchema.Type> = []
        for (const operation of [...operations].reverse()) {
          const postReceipt = receipt.postImageRevisionRefs.find(candidate => candidate.operationRef === operation.operationRef)!
          const postPath = operation._tag === "Rename" ? operation.targetPathRef : operation.pathRef
          const currentFile = yield* authority.snapshot(postPath)
          const exactPostImage = postReceipt.diskRevisionRef === null
            ? currentFile._tag === "Missing"
            : currentFile._tag === "File" && currentFile.file.diskRevisionRef === postReceipt.diskRevisionRef && currentFile.file.contentDigest === postReceipt.contentDigest
          if (!exactPostImage) {
            return yield* Effect.fail(new IdeAgentCodeBaseChanged({
              operation: "IdeAgentCode.undo",
              proposalRef: proposal.proposalRef,
              operationRef: operation.operationRef,
              pathRef: postPath,
              reason: "revision_changed",
            }))
          }
          const preimage = checkpoint.preimages.find(candidate => candidate.operationRef === operation.operationRef)!
          yield* authority.restore(operation, preimage, IdeAgentAuthorityPostImageSchema.make({
            operationRef: operation.operationRef,
            pathRef: postPath,
            diskRevisionRef: postReceipt.diskRevisionRef,
            contentDigest: postReceipt.contentDigest,
            documentRef: currentFile._tag === "File" ? currentFile.file.documentRef : null,
            documentGeneration: currentFile._tag === "File" ? currentFile.file.documentGeneration : null,
            encoding: currentFile._tag === "File" ? currentFile.file.encoding : null,
            lineEnding: currentFile._tag === "File" ? currentFile.file.lineEnding : null,
            mode: currentFile._tag === "File" ? currentFile.file.mode : null,
          }))
          restoredRefs.push(operation.operationRef)
        }
        const undoRef = IdeAgentUndoRefSchema.make(yield* nextRef("ide.agent-undo."))
        const undoReceipt = IdeAgentUndoReceiptSchema.make({
          undoRef,
          proposalRef: proposal.proposalRef,
          applyRef: input.applyRef,
          checkpointRef: input.checkpointRef,
          undoneAt: observedNow,
          restoredOperationRefs: restoredRefs,
        })
        const consumedCheckpoint = IdeAgentCheckpointSchema.make({ ...checkpoint, consumedByUndoRef: undoRef })
        const historicalBacklinks = current.backlinks.map(backlink => backlink.proposalRef !== proposal.proposalRef
          ? backlink
          : IdeAgentBacklinkSchema.make({
              ...backlink,
              resolution: {
                _tag: "Historical",
                checkpointRef: checkpoint.checkpointRef,
                pathRef: checkpoint.preimages.find(preimage => preimage.operationRef === backlink.operationRef)?.pathRef ?? "",
                contentDigest: checkpoint.preimages.find(preimage => preimage.operationRef === backlink.operationRef)?._tag === "File"
                  ? (checkpoint.preimages.find(preimage => preimage.operationRef === backlink.operationRef) as Extract<IdeAgentPreimage, { _tag: "File" }>).contentDigest
                  : `sha256:${"0".repeat(64)}`,
                range: null,
              },
            }))
        const staleEvidence = current.evidence.map(fact => fact.proposalRef !== proposal.proposalRef ? fact : IdeAgentEvidenceFactSchema.make({
          ...fact,
          state: IdeAgentEvidenceStateSchema.cases.Stale.make({
            observedAt: observedNow,
            reason: "The exact applied post-image was undone to its retained checkpoint.",
          }),
        }))
        const next = IdeAgentCodeSnapshotSchema.make({
          ...replaceProposal(current, IdeAgentProposalSchema.make({
            ...proposal,
            lifecycle: { _tag: "Undone", applyRef: input.applyRef, checkpointRef: input.checkpointRef, undoRef, undoneAt: observedNow },
          })),
          checkpoints: current.checkpoints.map(candidate => candidate.checkpointRef === checkpoint.checkpointRef ? consumedCheckpoint : candidate),
          undoReceipts: [...current.undoReceipts, undoReceipt].slice(-200),
          backlinks: historicalBacklinks,
          evidence: staleEvidence,
        })
        yield* Ref.set(state, next)
        return next
      }))
    })

    const recordEvidence = Effect.fn("IdeAgentCode.recordEvidence")(function* (
      raw: IdeAgentEvidenceFact,
      expectedAttachmentGeneration: number,
    ) {
      yield* ensureActive("IdeAgentCode.recordEvidence")
      const fact = yield* decodeInput("IdeAgentCode.recordEvidence", IdeAgentEvidenceFactSchema, raw)
      return yield* lock.withPermit(Effect.gen(function* () {
        const current = yield* Ref.get(state)
        yield* assertAttachment("IdeAgentCode.recordEvidence", current, expectedAttachmentGeneration)
        const proposal = yield* proposalAt("IdeAgentCode.recordEvidence", current, fact.proposalRef)
        if (proposal.lifecycle._tag !== "Applied" || proposal.lifecycle.applyRef !== fact.applyRef) {
          return yield* Effect.fail(new IdeAgentCodeProposalState({
            operation: "IdeAgentCode.recordEvidence",
            proposalRef: proposal.proposalRef,
            state: proposal.lifecycle._tag,
            detail: "post-apply evidence must bind the exact observed apply receipt",
          }))
        }
        if (JSON.stringify(fact.lineage) !== JSON.stringify(proposal.lineage)) {
          return yield* Effect.fail(new IdeAgentCodeInvariantViolation({
            operation: "IdeAgentCode.recordEvidence",
            detail: "post-apply evidence lineage must equal the exact proposal ProductSpec lineage",
          }))
        }
        const evidence = [...current.evidence.filter(candidate => candidate.evidenceRef !== fact.evidenceRef), fact].slice(-2_000)
        const next = IdeAgentCodeSnapshotSchema.make({ ...current, evidence, revision: current.revision + 1 })
        yield* Ref.set(state, next)
        return next
      }))
    })

    const receipt = Effect.fn("IdeAgentCode.receipt")(function* () {
      yield* ensureActive("IdeAgentCode.receipt")
      return safeReceipt(yield* Ref.get(state))
    })

    const stop = Effect.fn("IdeAgentCode.stop")(function* (reason: string) {
      yield* ensureActive("IdeAgentCode.stop")
      const boundedReason = reason.trim().slice(0, 1_000) || "stopped"
      const next = yield* Ref.updateAndGet(state, current => IdeAgentCodeSnapshotSchema.make({
        ...current,
        manifests: current.manifests.map(manifest => ({
          ...manifest,
          items: manifest.items.map(item => ({ ...item, excerpt: null })),
        })),
        checkpoints: [],
        lifecycle: "stopped",
        revision: current.revision + 1,
      }))
      yield* Ref.set(stopped, boundedReason)
      return next
    })

    yield* Effect.addFinalizer(() => Effect.gen(function* () {
      const reason = yield* Ref.get(stopped)
      if (reason !== null) return
      yield* Ref.update(state, current => IdeAgentCodeSnapshotSchema.make({
        ...current,
        manifests: current.manifests.map(manifest => ({
          ...manifest,
          items: manifest.items.map(item => ({ ...item, excerpt: null })),
        })),
        checkpoints: [],
        lifecycle: "stopped",
        revision: current.revision + 1,
      }))
      yield* Ref.set(stopped, "agent-code scope closed")
    }))

    return IdeAgentCodeService.of({
      snapshot,
      attach,
      assembleManifest,
      submitProposal,
      beginReview,
      decide,
      apply,
      rebase,
      undo,
      recordEvidence,
      receipt,
      stop,
    })
  }))

export const IdeAgentMemoryDocumentSchema = IdeAgentAuthorityFileSchema
export type IdeAgentMemoryDocument = typeof IdeAgentMemoryDocumentSchema.Type

export const makeIdeAgentMemoryAuthorityLayer = (
  seed: ReadonlyArray<IdeAgentMemoryDocument>,
): Layer.Layer<IdeAgentDocumentAuthority, IdeAgentCodeInvalidInput> =>
  Layer.effect(IdeAgentDocumentAuthority, Effect.gen(function* () {
    const decoded = yield* decodeInput(
      "IdeAgentMemoryAuthority.acquire",
      Schema.Array(IdeAgentMemoryDocumentSchema).check(Schema.isMaxLength(1_000)),
      seed,
    )
    const documents = yield* Ref.make(new Map(decoded.map(document => [document.pathRef, document])))
    const ordinal = yield* Ref.make(0)
    const lock = yield* Semaphore.make(1)

    const nextRevision = Effect.fn("IdeAgentMemoryAuthority.nextRevision")(function* () {
      const value = yield* Ref.updateAndGet(ordinal, current => current + 1)
      return IdeDiskRevisionRefSchema.make(`ide.disk-revision.memory.${value}`)
    })

    const snapshot = Effect.fn("IdeAgentMemoryAuthority.snapshot")(function* (pathRef: string) {
      const path = yield* decodeInput("IdeAgentMemoryAuthority.snapshot", DesktopWorkspacePathRefSchema, pathRef)
      const document = (yield* Ref.get(documents)).get(path)
      return document === undefined
        ? IdeAgentAuthoritySnapshotSchema.cases.Missing.make({ pathRef: path })
        : IdeAgentAuthoritySnapshotSchema.cases.File.make({ file: document })
    })
    const authoritySnapshot: IdeAgentDocumentAuthorityShape["snapshot"] = pathRef =>
      snapshot(pathRef).pipe(Effect.mapError(error => new IdeAgentAuthorityFailure({
        operation: "IdeAgentMemoryAuthority.snapshot",
        reason: "unavailable",
        message: error.detail,
      })))

    const apply = Effect.fn("IdeAgentMemoryAuthority.apply")(function* (operation: IdeAgentProposalOperation) {
      return yield* lock.withPermit(Effect.gen(function* () {
        const current = yield* Ref.get(documents)
        const next = new Map(current)
        const existing = current.get(operation.pathRef)
        const revision = yield* nextRevision()
        const generation = IdeDocumentGenerationSchema.make((existing?.documentGeneration ?? 0) + 1)
        let postImage: IdeAgentAuthorityPostImage
        switch (operation._tag) {
          case "Create": {
            if (existing !== undefined) return yield* Effect.fail(new IdeAgentAuthorityFailure({
              operation: "IdeAgentMemoryAuthority.apply.Create", reason: "conflict", message: "create target already exists",
            }))
            const documentRef = IdeDocumentRefSchema.make(`ide.document.agent.${operation.operationRef.split(".").at(-1)}`)
            next.set(operation.pathRef, IdeAgentAuthorityFileSchema.make({
              pathRef: operation.pathRef,
              fileRef: operation.fileRef,
              documentRef,
              documentGeneration: IdeDocumentGenerationSchema.make(1),
              diskRevisionRef: revision,
              content: operation.content,
              contentDigest: operation.contentDigest,
              encoding: operation.policy.encoding === "utf-8-bom" ? "utf-8-bom" : "utf-8",
              lineEnding: operation.policy.lineEnding === "crlf" ? "crlf" : "lf",
              mode: operation.policy.mode === "executable" ? "executable" : "regular",
              dirty: false,
              symlink: false,
              contentClass: "text",
            }))
            postImage = IdeAgentAuthorityPostImageSchema.make({
              operationRef: operation.operationRef, pathRef: operation.pathRef, diskRevisionRef: revision,
              contentDigest: operation.contentDigest, documentRef, documentGeneration: IdeDocumentGenerationSchema.make(1),
              encoding: operation.policy.encoding === "utf-8-bom" ? "utf-8-bom" : "utf-8",
              lineEnding: operation.policy.lineEnding === "crlf" ? "crlf" : "lf",
              mode: operation.policy.mode === "executable" ? "executable" : "regular",
            })
            break
          }
          case "Edit": {
            if (existing === undefined) return yield* Effect.fail(new IdeAgentAuthorityFailure({
              operation: "IdeAgentMemoryAuthority.apply.Edit", reason: "conflict", message: "edit target is missing",
            }))
            next.set(operation.pathRef, IdeAgentAuthorityFileSchema.make({
              ...existing,
              diskRevisionRef: revision,
              documentGeneration: generation,
              content: operation.targetContent,
              contentDigest: operation.targetContentDigest,
              lineEnding: operation.policy.lineEnding === "preserve" ? existing.lineEnding : operation.policy.lineEnding,
              mode: operation.policy.mode === "preserve" ? existing.mode : operation.policy.mode,
            }))
            postImage = IdeAgentAuthorityPostImageSchema.make({
              operationRef: operation.operationRef, pathRef: operation.pathRef, diskRevisionRef: revision,
              contentDigest: operation.targetContentDigest, documentRef: existing.documentRef, documentGeneration: generation,
              encoding: existing.encoding,
              lineEnding: operation.policy.lineEnding === "preserve" ? existing.lineEnding : operation.policy.lineEnding,
              mode: operation.policy.mode === "preserve" ? existing.mode : operation.policy.mode,
            })
            break
          }
          case "Rename": {
            if (existing === undefined || next.has(operation.targetPathRef)) return yield* Effect.fail(new IdeAgentAuthorityFailure({
              operation: "IdeAgentMemoryAuthority.apply.Rename", reason: "conflict", message: "rename base or target changed",
            }))
            next.delete(operation.pathRef)
            next.set(operation.targetPathRef, IdeAgentAuthorityFileSchema.make({
              ...existing, pathRef: operation.targetPathRef, diskRevisionRef: revision, documentGeneration: generation,
            }))
            postImage = IdeAgentAuthorityPostImageSchema.make({
              operationRef: operation.operationRef, pathRef: operation.targetPathRef, diskRevisionRef: revision,
              contentDigest: existing.contentDigest, documentRef: existing.documentRef, documentGeneration: generation,
              encoding: existing.encoding, lineEnding: existing.lineEnding, mode: existing.mode,
            })
            break
          }
          case "Delete": {
            if (existing === undefined) return yield* Effect.fail(new IdeAgentAuthorityFailure({
              operation: "IdeAgentMemoryAuthority.apply.Delete", reason: "conflict", message: "delete target is missing",
            }))
            next.delete(operation.pathRef)
            postImage = IdeAgentAuthorityPostImageSchema.make({
              operationRef: operation.operationRef, pathRef: operation.pathRef, diskRevisionRef: null,
              contentDigest: null, documentRef: null, documentGeneration: null,
              encoding: null, lineEnding: null, mode: null,
            })
            break
          }
        }
        yield* Ref.set(documents, next)
        return postImage
      }))
    })

    const restore = Effect.fn("IdeAgentMemoryAuthority.restore")(function* (
      operation: IdeAgentProposalOperation,
      preimage: IdeAgentPreimage,
      _postImage: IdeAgentAuthorityPostImage,
    ) {
      return yield* lock.withPermit(Effect.gen(function* () {
        const current = yield* Ref.get(documents)
        const next = new Map(current)
        const postPath = operation._tag === "Rename" ? operation.targetPathRef : operation.pathRef
        if (operation._tag === "Rename") next.delete(postPath)
        if (preimage._tag === "Missing") {
          next.delete(operation.pathRef)
          yield* Ref.set(documents, next)
          return IdeAgentAuthorityPostImageSchema.make({
            operationRef: operation.operationRef, pathRef: operation.pathRef, diskRevisionRef: null,
            contentDigest: null, documentRef: null, documentGeneration: null,
            encoding: null, lineEnding: null, mode: null,
          })
        }
        const existing = current.get(postPath) ?? current.get(operation.pathRef)
        const documentRef = existing?.documentRef ?? ("documentRef" in operation && operation.documentRef !== null
          ? operation.documentRef
          : IdeDocumentRefSchema.make(`ide.document.restored.${operation.operationRef.split(".").at(-1)}`))
        const file = IdeAgentAuthorityFileSchema.make({
          pathRef: preimage.pathRef,
          fileRef: operation.fileRef,
          documentRef,
          documentGeneration: IdeDocumentGenerationSchema.make((existing?.documentGeneration ?? operation.base.documentGeneration ?? 0) + 1),
          diskRevisionRef: preimage.diskRevisionRef,
          content: preimage.content,
          contentDigest: preimage.contentDigest,
          encoding: preimage.encoding,
          lineEnding: preimage.lineEnding,
          mode: preimage.mode,
          dirty: false,
          symlink: false,
          contentClass: "text",
        })
        next.set(preimage.pathRef, file)
        yield* Ref.set(documents, next)
        return IdeAgentAuthorityPostImageSchema.make({
          operationRef: operation.operationRef,
          pathRef: preimage.pathRef,
          diskRevisionRef: preimage.diskRevisionRef,
          contentDigest: preimage.contentDigest,
          documentRef: file.documentRef,
          documentGeneration: file.documentGeneration,
          encoding: file.encoding,
          lineEnding: file.lineEnding,
          mode: file.mode,
        })
      }))
    })

    return IdeAgentDocumentAuthority.of({ snapshot: authoritySnapshot, apply, restore })
  }))

export const makeIdeAgentCodeTestLayer = (
  documents: ReadonlyArray<IdeAgentMemoryDocument>,
  seed: unknown = emptyIdeAgentCodeSnapshot(),
  options: Readonly<{ now?: () => typeof IdeTimestampSchema.Type; checkpointRetentionMs?: number }> = {},
) => makeIdeAgentCodeLayer(seed, options).pipe(
  Layer.provide(makeIdeAgentMemoryAuthorityLayer(documents)),
)
