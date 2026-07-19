import { Exit, Schema } from "effect"

import { DesktopWorkspacePathRefSchema } from "../workspace-contract.ts"
import {
  IdeAttachmentGenerationSchema,
  IdeCheckpointRefSchema,
  IdeCommitRefSchema,
  IdeDiagnosticRefSchema,
  IdeDiskRevisionRefSchema,
  IdeDocumentGenerationSchema,
  IdeDocumentRefSchema,
  IdeEvidenceRefSchema,
  IdeFileRefSchema,
  IdeGitSnapshotGenerationSchema,
  IdeGitSnapshotRefSchema,
  IdePlacementGenerationSchema,
  IdePlacementRefSchema,
  IdeProjectRefSchema,
  IdeProposalRefSchema,
  IdeRootRefSchema,
  IdeSessionRefSchema,
  IdeSymbolRefSchema,
  IdeTextRangeSchema,
  IdeTimestampSchema,
  IdeWorktreeRefSchema,
} from "./project-contract.ts"

const boundedRef = <const Identifier extends string>(identifier: Identifier, prefix: string) =>
  Schema.String.pipe(
    Schema.check(
      Schema.isMinLength(prefix.length + 1),
      Schema.isMaxLength(192),
      Schema.isPattern(new RegExp(`^${prefix.replaceAll(".", "\\.")}[A-Za-z0-9][A-Za-z0-9._-]*$`, "u")),
    ),
    Schema.brand(identifier),
  ).annotate({ identifier })

const nonNegativeInteger = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0))
const positiveInteger = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1))
const boundedLabel = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(240))
const boundedDetail = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(1_000))
const contentDigest = Schema.String.check(Schema.isPattern(/^sha256:[a-f0-9]{64}$/u))

export const IdeAgentCodeSchemaVersion = Schema.Literal("openagents.desktop.ide-agent-code.v1")

export const IdeAgentAttachmentRefSchema = boundedRef("IdeAgentAttachmentRef", "ide.agent-attachment.")
export type IdeAgentAttachmentRef = typeof IdeAgentAttachmentRefSchema.Type

export const IdeAgentManifestRefSchema = boundedRef("IdeAgentManifestRef", "ide.agent-manifest.")
export type IdeAgentManifestRef = typeof IdeAgentManifestRefSchema.Type

export const IdeAgentContextItemRefSchema = boundedRef("IdeAgentContextItemRef", "ide.agent-context-item.")
export type IdeAgentContextItemRef = typeof IdeAgentContextItemRefSchema.Type

export const IdeAgentOperationRefSchema = boundedRef("IdeAgentOperationRef", "ide.agent-operation.")
export type IdeAgentOperationRef = typeof IdeAgentOperationRefSchema.Type

export const IdeAgentDecisionRefSchema = boundedRef("IdeAgentDecisionRef", "ide.agent-decision.")
export type IdeAgentDecisionRef = typeof IdeAgentDecisionRefSchema.Type

export const IdeAgentReviewRefSchema = boundedRef("IdeAgentReviewRef", "ide.agent-review.")
export type IdeAgentReviewRef = typeof IdeAgentReviewRefSchema.Type

export const IdeAgentApplyRefSchema = boundedRef("IdeAgentApplyRef", "ide.agent-apply.")
export type IdeAgentApplyRef = typeof IdeAgentApplyRefSchema.Type

export const IdeAgentUndoRefSchema = boundedRef("IdeAgentUndoRef", "ide.agent-undo.")
export type IdeAgentUndoRef = typeof IdeAgentUndoRefSchema.Type

export const IdeAgentBacklinkRefSchema = boundedRef("IdeAgentBacklinkRef", "ide.agent-backlink.")
export type IdeAgentBacklinkRef = typeof IdeAgentBacklinkRefSchema.Type

export const IdeAgentTurnRefSchema = boundedRef("IdeAgentTurnRef", "ide.agent-turn.")
export type IdeAgentTurnRef = typeof IdeAgentTurnRefSchema.Type

export const IdeAgentPacketRefSchema = boundedRef("IdeAgentPacketRef", "ide.agent-packet.")
export type IdeAgentPacketRef = typeof IdeAgentPacketRefSchema.Type

export const IdeAgentSpecRevisionRefSchema = boundedRef("IdeAgentSpecRevisionRef", "ide.agent-spec-revision.")
export type IdeAgentSpecRevisionRef = typeof IdeAgentSpecRevisionRefSchema.Type

export const IdeAgentAttachmentSchema = Schema.Struct({
  schemaVersion: IdeAgentCodeSchemaVersion,
  agentAttachmentRef: IdeAgentAttachmentRefSchema,
  projectRef: IdeProjectRefSchema,
  rootRef: IdeRootRefSchema,
  worktreeRef: IdeWorktreeRefSchema,
  sessionRef: IdeSessionRefSchema,
  attachmentGeneration: IdeAttachmentGenerationSchema,
  placementGeneration: IdePlacementGenerationSchema,
  grantRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(192)),
  attachedAt: IdeTimestampSchema,
  expiresAt: Schema.NullOr(IdeTimestampSchema),
}).annotate({ identifier: "IdeAgentAttachment" })
export type IdeAgentAttachment = typeof IdeAgentAttachmentSchema.Type

export const IdeAgentEffectiveRuntimeSchema = Schema.Struct({
  harnessRef: boundedLabel,
  modelRef: boundedLabel,
  providerRef: boundedLabel,
  accountRef: boundedLabel,
  placementRef: IdePlacementRefSchema,
  placementGeneration: IdePlacementGenerationSchema,
  toolPolicyRef: boundedLabel,
  permissionMode: Schema.Literals(["read_only", "proposal_only", "owner_full"]),
  sandboxRef: boundedLabel,
  memoryPolicyRef: boundedLabel,
  instructionPolicyRef: boundedLabel,
  semanticRetrieval: Schema.Literals(["disabled", "local", "managed_remote"]),
}).annotate({ identifier: "IdeAgentEffectiveRuntime" })
export type IdeAgentEffectiveRuntime = typeof IdeAgentEffectiveRuntimeSchema.Type

export const IdeAgentContextDestinationSchema = Schema.TaggedUnion({
  HarnessPrompt: { harnessRef: boundedLabel },
  ToolInput: { toolRef: boundedLabel },
  LocalMemory: { policyRef: boundedLabel },
  ManagedMemory: { placementRef: IdePlacementRefSchema, policyRef: boundedLabel },
  Withheld: { reason: boundedLabel },
}).annotate({ identifier: "IdeAgentContextDestination" })
export type IdeAgentContextDestination = typeof IdeAgentContextDestinationSchema.Type

export const IdeAgentContextDispositionSchema = Schema.TaggedUnion({
  Included: {
    reason: Schema.Literals([
      "explicit_user_selection", "active_file", "active_selection", "diagnostic_cause",
      "recent_edit", "git_cochange", "rule", "skill", "lexical_match", "symbol_match",
      "semantic_match", "runtime_policy",
    ]),
  },
  Omitted: {
    reason: Schema.Literals([
      "excluded_by_user", "unavailable", "stale", "over_budget", "secret", "private",
      "ignored", "binary", "too_large", "unsupported", "retrieval_disabled", "grant_revoked",
      "generation_replaced", "cancelled",
    ]),
    detail: boundedDetail,
  },
}).annotate({ identifier: "IdeAgentContextDisposition" })
export type IdeAgentContextDisposition = typeof IdeAgentContextDispositionSchema.Type

const contextSourceFields = {
  selectedBy: Schema.Literals(["user", "editor", "diagnostics", "git", "rule_engine", "skill", "retrieval", "runtime"]),
  // Monaco's initial in-memory document version is generation zero. The
  // context source must preserve that exact version instead of inventing a
  // successor generation at the renderer/main boundary.
  sourceGeneration: nonNegativeInteger,
}

export const IdeAgentContextSourceSchema = Schema.TaggedUnion({
  File: {
    ...contextSourceFields,
    fileRef: IdeFileRefSchema,
    documentRef: IdeDocumentRefSchema,
    pathRef: DesktopWorkspacePathRefSchema,
    documentGeneration: IdeDocumentGenerationSchema,
    diskRevisionRef: Schema.NullOr(IdeDiskRevisionRefSchema),
  },
  Range: {
    ...contextSourceFields,
    fileRef: IdeFileRefSchema,
    documentRef: IdeDocumentRefSchema,
    pathRef: DesktopWorkspacePathRefSchema,
    documentGeneration: IdeDocumentGenerationSchema,
    range: IdeTextRangeSchema,
  },
  Diagnostic: {
    ...contextSourceFields,
    diagnosticRef: IdeDiagnosticRefSchema,
    fileRef: IdeFileRefSchema,
    documentRef: IdeDocumentRefSchema,
    documentGeneration: IdeDocumentGenerationSchema,
  },
  Symbol: {
    ...contextSourceFields,
    symbolRef: IdeSymbolRefSchema,
    fileRef: IdeFileRefSchema,
    documentRef: IdeDocumentRefSchema,
    documentGeneration: IdeDocumentGenerationSchema,
  },
  GitChange: {
    ...contextSourceFields,
    fileRef: IdeFileRefSchema,
    pathRef: DesktopWorkspacePathRefSchema,
    gitSnapshotRef: IdeGitSnapshotRefSchema,
    gitSnapshotGeneration: IdeGitSnapshotGenerationSchema,
  },
  Rule: { ...contextSourceFields, ruleRef: boundedLabel },
  Skill: { ...contextSourceFields, skillRef: boundedLabel },
  RecentEdit: {
    ...contextSourceFields,
    fileRef: IdeFileRefSchema,
    documentRef: IdeDocumentRefSchema,
    documentGeneration: IdeDocumentGenerationSchema,
  },
  LexicalRetrieval: { ...contextSourceFields, resultRef: boundedLabel, queryDigest: contentDigest },
  SemanticRetrieval: { ...contextSourceFields, resultRef: boundedLabel, queryDigest: contentDigest },
  RuntimePolicy: { ...contextSourceFields, policyRef: boundedLabel },
  Unavailable: {
    ...contextSourceFields,
    sourceClass: Schema.Literals([
      "active_selection", "diagnostics", "symbols", "git_cochange", "rule", "skill",
      "recent_edit", "lexical_retrieval", "semantic_retrieval", "runtime_policy",
    ]),
    detail: boundedDetail,
  },
}).annotate({ identifier: "IdeAgentContextSource" })
export type IdeAgentContextSource = typeof IdeAgentContextSourceSchema.Type

/**
 * Monaco owns a zero-based model-incarnation clock. Project/workspace
 * authority owns a one-based document clock. Agent-code is the boundary that
 * relates the two, so the translation lives beside its schema graph.
 */
export const projectDocumentGenerationForSource = (sourceGeneration: number) =>
  IdeDocumentGenerationSchema.make(sourceGeneration + 1)

export const IdeAgentContextItemSchema = Schema.Struct({
  contextItemRef: IdeAgentContextItemRefSchema,
  source: IdeAgentContextSourceSchema,
  disposition: IdeAgentContextDispositionSchema,
  destination: IdeAgentContextDestinationSchema,
  freshness: Schema.Literals(["current", "stale", "historical", "unavailable"]),
  sensitivity: Schema.Literals(["public", "workspace", "private", "secret"]),
  retention: Schema.Literals(["turn_only", "session", "local_rebuildable", "managed_retained", "withheld"]),
  byteEstimate: nonNegativeInteger,
  tokenEstimate: nonNegativeInteger,
  truncated: Schema.Boolean,
  label: boundedLabel,
  excerpt: Schema.NullOr(Schema.String.check(Schema.isMaxLength(64_000))),
}).annotate({ identifier: "IdeAgentContextItem" })
export type IdeAgentContextItem = typeof IdeAgentContextItemSchema.Type

export const IdeAgentContextManifestSchema = Schema.Struct({
  schemaVersion: IdeAgentCodeSchemaVersion,
  manifestRef: IdeAgentManifestRefSchema,
  attachment: IdeAgentAttachmentSchema,
  turnRef: IdeAgentTurnRefSchema,
  conversationThreadRef: Schema.NullOr(boundedLabel),
  createdAt: IdeTimestampSchema,
  effectiveRuntime: IdeAgentEffectiveRuntimeSchema,
  items: Schema.Array(IdeAgentContextItemSchema).check(Schema.isMaxLength(2_000)),
  includedBytes: nonNegativeInteger,
  includedTokens: nonNegativeInteger,
  omittedCount: nonNegativeInteger,
  byteBudget: positiveInteger,
  tokenBudget: positiveInteger,
  exportable: Schema.Boolean,
  rebuildable: Schema.Boolean,
  deletionPolicyRef: boundedLabel,
}).annotate({ identifier: "IdeAgentContextManifest" })
export type IdeAgentContextManifest = typeof IdeAgentContextManifestSchema.Type

export const IdeAgentFilePolicySchema = Schema.Struct({
  encoding: Schema.Literals(["preserve", "utf-8", "utf-8-bom"]),
  lineEnding: Schema.Literals(["preserve", "lf", "crlf"]),
  mode: Schema.Literals(["preserve", "regular", "executable"]),
  symlink: Schema.Literals(["refuse", "preserve_target"]),
}).annotate({ identifier: "IdeAgentFilePolicy" })
export type IdeAgentFilePolicy = typeof IdeAgentFilePolicySchema.Type

export const IdeAgentProposalBaseSchema = Schema.Struct({
  existed: Schema.Boolean,
  /** Exact admitted review preimage; private persistence, never public receipt material. */
  content: Schema.NullOr(Schema.String.check(Schema.isMaxLength(1_000_000))),
  diskRevisionRef: Schema.NullOr(IdeDiskRevisionRefSchema),
  documentRef: Schema.NullOr(IdeDocumentRefSchema),
  documentGeneration: Schema.NullOr(IdeDocumentGenerationSchema),
  gitSnapshotRef: Schema.NullOr(IdeGitSnapshotRefSchema),
  gitSnapshotGeneration: Schema.NullOr(IdeGitSnapshotGenerationSchema),
  checkpointRef: Schema.NullOr(IdeCheckpointRefSchema),
  contentDigest: Schema.NullOr(contentDigest),
  encoding: Schema.Literals(["utf-8", "utf-8-bom", "none"]),
  lineEnding: Schema.Literals(["lf", "crlf", "mixed", "none"]),
  mode: Schema.Literals(["regular", "executable", "none"]),
}).annotate({ identifier: "IdeAgentProposalBase" })
export type IdeAgentProposalBase = typeof IdeAgentProposalBaseSchema.Type

const operationFields = {
  operationRef: IdeAgentOperationRefSchema,
  fileRef: IdeFileRefSchema,
  pathRef: DesktopWorkspacePathRefSchema,
  base: IdeAgentProposalBaseSchema,
  policy: IdeAgentFilePolicySchema,
}

export const IdeAgentProposalOperationSchema = Schema.TaggedUnion({
  Create: {
    ...operationFields,
    content: Schema.String.check(Schema.isMaxLength(1_000_000)),
    contentDigest,
  },
  Edit: {
    ...operationFields,
    documentRef: IdeDocumentRefSchema,
    targetContent: Schema.String.check(Schema.isMaxLength(1_000_000)),
    targetContentDigest: contentDigest,
  },
  Rename: {
    ...operationFields,
    documentRef: Schema.NullOr(IdeDocumentRefSchema),
    targetPathRef: DesktopWorkspacePathRefSchema,
  },
  Delete: {
    ...operationFields,
    documentRef: Schema.NullOr(IdeDocumentRefSchema),
  },
}).annotate({ identifier: "IdeAgentProposalOperation" })
export type IdeAgentProposalOperation = typeof IdeAgentProposalOperationSchema.Type

export const IdeAgentProposalLifecycleSchema = Schema.TaggedUnion({
  Pending: {},
  Reviewing: { reviewRef: IdeAgentReviewRefSchema },
  PartiallyAccepted: {
    acceptedOperationRefs: Schema.Array(IdeAgentOperationRefSchema).check(Schema.isMinLength(1), Schema.isMaxLength(200)),
    rejectedOperationRefs: Schema.Array(IdeAgentOperationRefSchema).check(Schema.isMaxLength(200)),
    childProposalRef: IdeProposalRefSchema,
  },
  Accepted: { acceptedOperationRefs: Schema.Array(IdeAgentOperationRefSchema).check(Schema.isMinLength(1), Schema.isMaxLength(200)) },
  Applying: { applyRef: IdeAgentApplyRefSchema, checkpointRef: IdeCheckpointRefSchema },
  Applied: { applyRef: IdeAgentApplyRefSchema, checkpointRef: IdeCheckpointRefSchema, undoableUntil: IdeTimestampSchema },
  Undone: { applyRef: IdeAgentApplyRefSchema, checkpointRef: IdeCheckpointRefSchema, undoRef: IdeAgentUndoRefSchema, undoneAt: IdeTimestampSchema },
  Rejected: { decisionRef: IdeAgentDecisionRefSchema, reason: boundedDetail },
  RebaseRequired: {
    reason: Schema.Literals([
      "base_changed", "external_change", "dirty_document", "attachment_changed", "created", "deleted",
      "revision_changed", "document_changed", "symlink", "binary", "secret", "private", "too_large",
      "unsupported_policy", "unavailable",
    ]),
    conflictCount: nonNegativeInteger,
    currentPathRef: DesktopWorkspacePathRefSchema,
    currentState: Schema.Literals(["file", "missing", "unavailable"]),
    currentDiskRevisionRef: Schema.NullOr(IdeDiskRevisionRefSchema),
    currentDocumentGeneration: Schema.NullOr(IdeDocumentGenerationSchema),
    currentContentDigest: Schema.NullOr(contentDigest),
  },
  Superseded: { replacementProposalRef: IdeProposalRefSchema },
  Cancelled: { reason: boundedDetail },
  Failed: { reason: boundedDetail, recoverable: Schema.Boolean },
}).annotate({ identifier: "IdeAgentProposalLifecycle" })
export type IdeAgentProposalLifecycle = typeof IdeAgentProposalLifecycleSchema.Type

export const IdeAgentProductSpecLineageSchema = Schema.Struct({
  specRevisionRef: IdeAgentSpecRevisionRefSchema,
  specDigest: contentDigest,
  criterionId: boundedLabel,
  packetRef: IdeAgentPacketRefSchema,
  terminalOutcome: Schema.Literals(["pending", "passed", "failed", "blocked", "disposed"]),
  reviewPostImageRef: Schema.NullOr(IdeEvidenceRefSchema),
}).annotate({ identifier: "IdeAgentProductSpecLineage" })
export type IdeAgentProductSpecLineage = typeof IdeAgentProductSpecLineageSchema.Type

export const IdeAgentProposalSchema = Schema.Struct({
  schemaVersion: IdeAgentCodeSchemaVersion,
  proposalRef: IdeProposalRefSchema,
  parentProposalRef: Schema.NullOr(IdeProposalRefSchema),
  attachment: IdeAgentAttachmentSchema,
  manifestRef: IdeAgentManifestRefSchema,
  sessionRef: IdeSessionRefSchema,
  turnRef: IdeAgentTurnRefSchema,
  conversationThreadRef: Schema.NullOr(boundedLabel),
  createdAt: IdeTimestampSchema,
  operations: Schema.Array(IdeAgentProposalOperationSchema).check(Schema.isMinLength(1), Schema.isMaxLength(200)),
  lifecycle: IdeAgentProposalLifecycleSchema,
  lineage: Schema.NullOr(IdeAgentProductSpecLineageSchema),
}).annotate({ identifier: "IdeAgentProposal" })
export type IdeAgentProposal = typeof IdeAgentProposalSchema.Type

export const IdeAgentDecisionSchema = Schema.Struct({
  decisionRef: IdeAgentDecisionRefSchema,
  proposalRef: IdeProposalRefSchema,
  decidedAt: IdeTimestampSchema,
  disposition: Schema.Literals(["accept", "reject"]),
  operationRefs: Schema.Array(IdeAgentOperationRefSchema).check(Schema.isMinLength(1), Schema.isMaxLength(200)),
  reason: Schema.NullOr(Schema.String.check(Schema.isMaxLength(1_000))),
}).annotate({ identifier: "IdeAgentDecision" })
export type IdeAgentDecision = typeof IdeAgentDecisionSchema.Type

export const IdeAgentPreimageSchema = Schema.TaggedUnion({
  Missing: { operationRef: IdeAgentOperationRefSchema, pathRef: DesktopWorkspacePathRefSchema },
  File: {
    operationRef: IdeAgentOperationRefSchema,
    pathRef: DesktopWorkspacePathRefSchema,
    content: Schema.String.check(Schema.isMaxLength(1_000_000)),
    contentDigest,
    diskRevisionRef: IdeDiskRevisionRefSchema,
    encoding: Schema.Literals(["utf-8", "utf-8-bom"]),
    lineEnding: Schema.Literals(["lf", "crlf", "mixed", "none"]),
    mode: Schema.Literals(["regular", "executable"]),
  },
}).annotate({ identifier: "IdeAgentPreimage" })
export type IdeAgentPreimage = typeof IdeAgentPreimageSchema.Type

export const IdeAgentCheckpointSchema = Schema.Struct({
  checkpointRef: IdeCheckpointRefSchema,
  proposalRef: IdeProposalRefSchema,
  attachment: IdeAgentAttachmentSchema,
  createdAt: IdeTimestampSchema,
  expiresAt: IdeTimestampSchema,
  preimages: Schema.Array(IdeAgentPreimageSchema).check(Schema.isMinLength(1), Schema.isMaxLength(200)),
  consumedByUndoRef: Schema.NullOr(IdeAgentUndoRefSchema),
}).annotate({ identifier: "IdeAgentCheckpoint" })
export type IdeAgentCheckpoint = typeof IdeAgentCheckpointSchema.Type

export const IdeAgentApplyReceiptSchema = Schema.Struct({
  applyRef: IdeAgentApplyRefSchema,
  proposalRef: IdeProposalRefSchema,
  checkpointRef: IdeCheckpointRefSchema,
  attachment: IdeAgentAttachmentSchema,
  appliedAt: IdeTimestampSchema,
  operationRefs: Schema.Array(IdeAgentOperationRefSchema).check(Schema.isMinLength(1), Schema.isMaxLength(200)),
  postImageRevisionRefs: Schema.Array(Schema.Struct({
    operationRef: IdeAgentOperationRefSchema,
    pathRef: DesktopWorkspacePathRefSchema,
    diskRevisionRef: Schema.NullOr(IdeDiskRevisionRefSchema),
    contentDigest: Schema.NullOr(contentDigest),
    encoding: Schema.NullOr(Schema.Literals(["utf-8", "utf-8-bom"])),
    lineEnding: Schema.NullOr(Schema.Literals(["lf", "crlf", "mixed", "none"])),
    mode: Schema.NullOr(Schema.Literals(["regular", "executable"])),
  })).check(Schema.isMaxLength(200)),
  rollback: Schema.Literals(["not_needed", "completed", "failed"]),
  undoableUntil: IdeTimestampSchema,
}).annotate({ identifier: "IdeAgentApplyReceipt" })
export type IdeAgentApplyReceipt = typeof IdeAgentApplyReceiptSchema.Type

export const IdeAgentUndoReceiptSchema = Schema.Struct({
  undoRef: IdeAgentUndoRefSchema,
  proposalRef: IdeProposalRefSchema,
  applyRef: IdeAgentApplyRefSchema,
  checkpointRef: IdeCheckpointRefSchema,
  undoneAt: IdeTimestampSchema,
  restoredOperationRefs: Schema.Array(IdeAgentOperationRefSchema).check(Schema.isMinLength(1), Schema.isMaxLength(200)),
}).annotate({ identifier: "IdeAgentUndoReceipt" })
export type IdeAgentUndoReceipt = typeof IdeAgentUndoReceiptSchema.Type

export const IdeAgentBacklinkResolutionSchema = Schema.TaggedUnion({
  Current: {
    fileRef: IdeFileRefSchema,
    documentRef: IdeDocumentRefSchema,
    documentGeneration: IdeDocumentGenerationSchema,
    pathRef: DesktopWorkspacePathRefSchema,
    range: Schema.NullOr(IdeTextRangeSchema),
  },
  Historical: {
    checkpointRef: IdeCheckpointRefSchema,
    pathRef: DesktopWorkspacePathRefSchema,
    contentDigest,
    range: Schema.NullOr(IdeTextRangeSchema),
  },
  Unavailable: {
    reason: Schema.Literals(["generation_replaced", "retention_expired", "deleted", "grant_revoked", "corrupt_persistence"]),
  },
}).annotate({ identifier: "IdeAgentBacklinkResolution" })
export type IdeAgentBacklinkResolution = typeof IdeAgentBacklinkResolutionSchema.Type

export const IdeAgentBacklinkSchema = Schema.Struct({
  backlinkRef: IdeAgentBacklinkRefSchema,
  proposalRef: IdeProposalRefSchema,
  operationRef: IdeAgentOperationRefSchema,
  sessionRef: IdeSessionRefSchema,
  turnRef: IdeAgentTurnRefSchema,
  conversationThreadRef: Schema.NullOr(boundedLabel),
  attachmentGeneration: IdeAttachmentGenerationSchema,
  createdAt: IdeTimestampSchema,
  resolution: IdeAgentBacklinkResolutionSchema,
}).annotate({ identifier: "IdeAgentBacklink" })
export type IdeAgentBacklink = typeof IdeAgentBacklinkSchema.Type

export const IdeAgentEvidenceStateSchema = Schema.TaggedUnion({
  Requested: { requestedAt: IdeTimestampSchema },
  Running: { startedAt: IdeTimestampSchema },
  Passed: { observedAt: IdeTimestampSchema, summary: boundedDetail },
  Failed: { observedAt: IdeTimestampSchema, summary: boundedDetail },
  Unavailable: { observedAt: IdeTimestampSchema, reason: boundedDetail },
  Stale: { observedAt: IdeTimestampSchema, reason: boundedDetail },
}).annotate({ identifier: "IdeAgentEvidenceState" })
export type IdeAgentEvidenceState = typeof IdeAgentEvidenceStateSchema.Type

export const IdeAgentEvidenceFactSchema = Schema.Struct({
  evidenceRef: IdeEvidenceRefSchema,
  proposalRef: IdeProposalRefSchema,
  applyRef: IdeAgentApplyRefSchema,
  postImageGeneration: positiveInteger,
  kind: Schema.Literals(["diagnostics", "format", "test", "git_status", "git_diff", "commit", "push", "pull_request", "delivery", "verification", "acceptance"]),
  state: IdeAgentEvidenceStateSchema,
  observedBy: Schema.Literals(["document_authority", "language_service", "task_service", "git_service", "delivery_service", "owner", "independent_reviewer"]),
  artifactRef: Schema.NullOr(boundedLabel),
  commitRef: Schema.NullOr(IdeCommitRefSchema),
  lineage: Schema.NullOr(IdeAgentProductSpecLineageSchema),
}).annotate({ identifier: "IdeAgentEvidenceFact" })
export type IdeAgentEvidenceFact = typeof IdeAgentEvidenceFactSchema.Type

export const IdeAgentCodeSnapshotSchema = Schema.Struct({
  schemaVersion: IdeAgentCodeSchemaVersion,
  attachment: Schema.NullOr(IdeAgentAttachmentSchema),
  manifests: Schema.Array(IdeAgentContextManifestSchema).check(Schema.isMaxLength(100)),
  proposals: Schema.Array(IdeAgentProposalSchema).check(Schema.isMaxLength(200)),
  decisions: Schema.Array(IdeAgentDecisionSchema).check(Schema.isMaxLength(500)),
  checkpoints: Schema.Array(IdeAgentCheckpointSchema).check(Schema.isMaxLength(200)),
  applyReceipts: Schema.Array(IdeAgentApplyReceiptSchema).check(Schema.isMaxLength(200)),
  undoReceipts: Schema.Array(IdeAgentUndoReceiptSchema).check(Schema.isMaxLength(200)),
  backlinks: Schema.Array(IdeAgentBacklinkSchema).check(Schema.isMaxLength(1_000)),
  evidence: Schema.Array(IdeAgentEvidenceFactSchema).check(Schema.isMaxLength(2_000)),
  lifecycle: Schema.Literals(["unattached", "attached", "stopped"]),
  revision: nonNegativeInteger,
}).annotate({ identifier: "IdeAgentCodeSnapshot" })
export type IdeAgentCodeSnapshot = typeof IdeAgentCodeSnapshotSchema.Type

export const emptyIdeAgentCodeSnapshot = (): IdeAgentCodeSnapshot => ({
  schemaVersion: "openagents.desktop.ide-agent-code.v1",
  attachment: null,
  manifests: [],
  proposals: [],
  decisions: [],
  checkpoints: [],
  applyReceipts: [],
  undoReceipts: [],
  backlinks: [],
  evidence: [],
  lifecycle: "unattached",
  revision: 0,
})

export const IdeAgentContextAssemblyInputSchema = Schema.Struct({
  manifest: IdeAgentContextManifestSchema,
  expectedAttachmentGeneration: IdeAttachmentGenerationSchema,
}).annotate({ identifier: "IdeAgentContextAssemblyInput" })
export type IdeAgentContextAssemblyInput = typeof IdeAgentContextAssemblyInputSchema.Type

export const IdeAgentProposalInputSchema = Schema.Struct({
  proposal: IdeAgentProposalSchema,
  expectedAttachmentGeneration: IdeAttachmentGenerationSchema,
}).annotate({ identifier: "IdeAgentProposalInput" })
export type IdeAgentProposalInput = typeof IdeAgentProposalInputSchema.Type

export const IdeAgentReviewInputSchema = Schema.Struct({
  proposalRef: IdeProposalRefSchema,
  reviewRef: IdeAgentReviewRefSchema,
  expectedAttachmentGeneration: IdeAttachmentGenerationSchema,
}).annotate({ identifier: "IdeAgentReviewInput" })
export type IdeAgentReviewInput = typeof IdeAgentReviewInputSchema.Type

export const IdeAgentApplyInputSchema = Schema.Struct({
  proposalRef: IdeProposalRefSchema,
  operationRefs: Schema.Array(IdeAgentOperationRefSchema).check(Schema.isMinLength(1), Schema.isMaxLength(200)),
  expectedAttachmentGeneration: IdeAttachmentGenerationSchema,
  expectedProposalRevision: nonNegativeInteger,
}).annotate({ identifier: "IdeAgentApplyInput" })
export type IdeAgentApplyInput = typeof IdeAgentApplyInputSchema.Type

export const IdeAgentRebaseInputSchema = Schema.Struct({
  proposalRef: IdeProposalRefSchema,
  replacementProposal: IdeAgentProposalSchema,
  expectedAttachmentGeneration: IdeAttachmentGenerationSchema,
}).annotate({ identifier: "IdeAgentRebaseInput" })
export type IdeAgentRebaseInput = typeof IdeAgentRebaseInputSchema.Type

export const IdeAgentUndoInputSchema = Schema.Struct({
  proposalRef: IdeProposalRefSchema,
  applyRef: IdeAgentApplyRefSchema,
  checkpointRef: IdeCheckpointRefSchema,
  expectedAttachmentGeneration: IdeAttachmentGenerationSchema,
}).annotate({ identifier: "IdeAgentUndoInput" })
export type IdeAgentUndoInput = typeof IdeAgentUndoInputSchema.Type

export const IdeAgentCodeReceiptSchema = Schema.Struct({
  schemaVersion: IdeAgentCodeSchemaVersion,
  lifecycle: Schema.Literals(["unattached", "attached", "stopped"]),
  attachmentRef: Schema.NullOr(IdeAgentAttachmentRefSchema),
  projectRef: Schema.NullOr(IdeProjectRefSchema),
  worktreeRef: Schema.NullOr(IdeWorktreeRefSchema),
  attachmentGeneration: Schema.NullOr(IdeAttachmentGenerationSchema),
  manifestCount: nonNegativeInteger,
  includedItemCount: nonNegativeInteger,
  omittedItemCount: nonNegativeInteger,
  proposalCounts: Schema.Struct({
    pending: nonNegativeInteger,
    reviewing: nonNegativeInteger,
    partial: nonNegativeInteger,
    applied: nonNegativeInteger,
    undone: nonNegativeInteger,
    refused: nonNegativeInteger,
    stale: nonNegativeInteger,
  }),
  checkpointCount: nonNegativeInteger,
  backlinkCount: nonNegativeInteger,
  evidenceCounts: Schema.Struct({ observed: nonNegativeInteger, passed: nonNegativeInteger, failed: nonNegativeInteger }),
  containsPrivateContent: Schema.Literal(false),
}).annotate({ identifier: "IdeAgentCodeReceipt" })
export type IdeAgentCodeReceipt = typeof IdeAgentCodeReceiptSchema.Type

export const DesktopIdeAgentCodeSnapshotChannel = "openagents-desktop/ide-agent-code-snapshot" as const
export const DesktopIdeAgentCodeCommandChannel = "openagents-desktop/ide-agent-code-command" as const

export const IdeAgentCodeCommandSchema = Schema.TaggedUnion({
  Attach: { attachment: IdeAgentAttachmentSchema },
  AssembleManifest: { input: IdeAgentContextAssemblyInputSchema },
  SubmitProposal: { input: IdeAgentProposalInputSchema },
  BeginReview: { input: IdeAgentReviewInputSchema },
  Decide: { decision: IdeAgentDecisionSchema, expectedAttachmentGeneration: IdeAttachmentGenerationSchema },
  Apply: { input: IdeAgentApplyInputSchema },
  Rebase: { input: IdeAgentRebaseInputSchema },
  Undo: { input: IdeAgentUndoInputSchema },
  Stop: { reason: boundedDetail },
}).annotate({ identifier: "IdeAgentCodeCommand" })
export type IdeAgentCodeCommand = typeof IdeAgentCodeCommandSchema.Type

export const IdeAgentCodeCommandResultSchema = Schema.TaggedUnion({
  Succeeded: { snapshot: IdeAgentCodeSnapshotSchema },
  Refused: {
    reason: Schema.Literals([
      "invalid_input", "unattached", "stopped", "wrong_attachment", "stale_generation", "manifest_missing",
      "proposal_missing", "proposal_state", "base_changed", "dirty_document", "conflict", "unsupported_policy",
      "grant_revoked", "unavailable", "rollback_failed", "checkpoint_expired", "corrupt_persistence",
    ]),
    message: boundedDetail,
    snapshot: IdeAgentCodeSnapshotSchema,
  },
}).annotate({ identifier: "IdeAgentCodeCommandResult" })
export type IdeAgentCodeCommandResult = typeof IdeAgentCodeCommandResultSchema.Type

const decodeOrNull = <S extends Schema.ConstraintDecoder<unknown, never>>(schema: S, value: unknown): S["Type"] | null => {
  const result = Schema.decodeUnknownExit(schema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeIdeAgentCodeSnapshot = (value: unknown): IdeAgentCodeSnapshot | null =>
  decodeOrNull(IdeAgentCodeSnapshotSchema, value)

export const decodeIdeAgentCodeCommand = (value: unknown): IdeAgentCodeCommand | null =>
  decodeOrNull(IdeAgentCodeCommandSchema, value)

export const decodeIdeAgentCodeCommandResult = (value: unknown): IdeAgentCodeCommandResult | null =>
  decodeOrNull(IdeAgentCodeCommandResultSchema, value)

export const decodeIdeAgentContextManifest = (value: unknown): IdeAgentContextManifest | null =>
  decodeOrNull(IdeAgentContextManifestSchema, value)

export const decodeIdeAgentProposal = (value: unknown): IdeAgentProposal | null =>
  decodeOrNull(IdeAgentProposalSchema, value)
