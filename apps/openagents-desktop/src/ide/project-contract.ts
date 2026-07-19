import { Schema } from "effect";

import { DesktopWorkspacePathRefSchema } from "../workspace-contract.ts";

const boundedRef = <const Identifier extends string>(identifier: Identifier, prefix: string) =>
  Schema.String.pipe(
    Schema.check(
      Schema.isMinLength(prefix.length + 1),
      Schema.isMaxLength(192),
      Schema.isPattern(
        new RegExp(`^${prefix.replaceAll(".", "\\.")}[A-Za-z0-9][A-Za-z0-9._-]*$`, "u"),
      ),
    ),
    Schema.brand(identifier),
  ).annotate({ identifier });

const positiveGeneration = <const Identifier extends string>(identifier: Identifier) =>
  Schema.Number.pipe(
    Schema.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
    Schema.brand(identifier),
  ).annotate({ identifier });

export const IdeProjectSchemaVersion = Schema.Literal("openagents.desktop.ide-project.v1");

export const IdeProjectRefSchema = boundedRef("IdeProjectRef", "ide.project.");
export type IdeProjectRef = typeof IdeProjectRefSchema.Type;

export const IdeRootRefSchema = boundedRef("IdeRootRef", "ide.root.");
export type IdeRootRef = typeof IdeRootRefSchema.Type;

export const IdeWorktreeRefSchema = boundedRef("IdeWorktreeRef", "ide.worktree.");
export type IdeWorktreeRef = typeof IdeWorktreeRefSchema.Type;

export const IdeFileRefSchema = boundedRef("IdeFileRef", "ide.file.");
export type IdeFileRef = typeof IdeFileRefSchema.Type;

export const IdeDocumentRefSchema = boundedRef("IdeDocumentRef", "ide.document.");
export type IdeDocumentRef = typeof IdeDocumentRefSchema.Type;

export const IdeAttachmentRefSchema = boundedRef("IdeAttachmentRef", "ide.attachment.");
export type IdeAttachmentRef = typeof IdeAttachmentRefSchema.Type;

export const IdeSessionRefSchema = boundedRef("IdeSessionRef", "ide.session.");
export type IdeSessionRef = typeof IdeSessionRefSchema.Type;

export const IdeCapabilityRefSchema = boundedRef("IdeCapabilityRef", "ide.capability.");
export type IdeCapabilityRef = typeof IdeCapabilityRefSchema.Type;

export const IdeNavigationRefSchema = boundedRef("IdeNavigationRef", "ide.navigation.");
export type IdeNavigationRef = typeof IdeNavigationRefSchema.Type;

export const IdeExcerptRefSchema = boundedRef("IdeExcerptRef", "ide.excerpt.");
export type IdeExcerptRef = typeof IdeExcerptRefSchema.Type;

export const IdeDiagnosticRefSchema = boundedRef("IdeDiagnosticRef", "ide.diagnostic.");
export type IdeDiagnosticRef = typeof IdeDiagnosticRefSchema.Type;

export const IdeSymbolRefSchema = boundedRef("IdeSymbolRef", "ide.symbol.");
export type IdeSymbolRef = typeof IdeSymbolRefSchema.Type;

export const IdeReviewRefSchema = boundedRef("IdeReviewRef", "ide.review.");
export type IdeReviewRef = typeof IdeReviewRefSchema.Type;

export const IdeProposalRefSchema = boundedRef("IdeProposalRef", "ide.proposal.");
export type IdeProposalRef = typeof IdeProposalRefSchema.Type;

export const IdeEvidenceRefSchema = boundedRef("IdeEvidenceRef", "ide.evidence.");
export type IdeEvidenceRef = typeof IdeEvidenceRefSchema.Type;

export const IdePlacementRefSchema = boundedRef("IdePlacementRef", "ide.placement.");
export type IdePlacementRef = typeof IdePlacementRefSchema.Type;

export const IdeDiskRevisionRefSchema = boundedRef("IdeDiskRevisionRef", "ide.disk-revision.");
export type IdeDiskRevisionRef = typeof IdeDiskRevisionRefSchema.Type;

export const IdeGitSnapshotRefSchema = boundedRef("IdeGitSnapshotRef", "ide.git-snapshot.");
export type IdeGitSnapshotRef = typeof IdeGitSnapshotRefSchema.Type;

export const IdeCommitRefSchema = boundedRef("IdeCommitRef", "ide.commit.");
export type IdeCommitRef = typeof IdeCommitRefSchema.Type;

export const IdeCheckpointRefSchema = boundedRef("IdeCheckpointRef", "ide.checkpoint.");
export type IdeCheckpointRef = typeof IdeCheckpointRefSchema.Type;

export const IdeEditorGroupRefSchema = boundedRef("IdeEditorGroupRef", "ide.editor-group.");
export type IdeEditorGroupRef = typeof IdeEditorGroupRefSchema.Type;

export const IdeLanguageServiceRefSchema = boundedRef(
  "IdeLanguageServiceRef",
  "ide.language-service.",
);
export type IdeLanguageServiceRef = typeof IdeLanguageServiceRefSchema.Type;

export const IdeSearchRequestRefSchema = boundedRef("IdeSearchRequestRef", "ide.search-request.");
export type IdeSearchRequestRef = typeof IdeSearchRequestRefSchema.Type;

export const IdeTerminalRefSchema = boundedRef("IdeTerminalRef", "ide.terminal.");
export type IdeTerminalRef = typeof IdeTerminalRefSchema.Type;

export const IdeTaskRefSchema = boundedRef("IdeTaskRef", "ide.task.");
export type IdeTaskRef = typeof IdeTaskRefSchema.Type;

export const IdeDebugSessionRefSchema = boundedRef("IdeDebugSessionRef", "ide.debug-session.");
export type IdeDebugSessionRef = typeof IdeDebugSessionRefSchema.Type;

export const IdeSettingRevisionRefSchema = boundedRef(
  "IdeSettingRevisionRef",
  "ide.setting-revision.",
);
export type IdeSettingRevisionRef = typeof IdeSettingRevisionRefSchema.Type;

export const IdeKeybindingRevisionRefSchema = boundedRef(
  "IdeKeybindingRevisionRef",
  "ide.keybinding-revision.",
);
export type IdeKeybindingRevisionRef = typeof IdeKeybindingRevisionRefSchema.Type;

export const IdeAttachmentGenerationSchema = positiveGeneration("IdeAttachmentGeneration");
export type IdeAttachmentGeneration = typeof IdeAttachmentGenerationSchema.Type;

export const IdeDocumentGenerationSchema = positiveGeneration("IdeDocumentGeneration");
export type IdeDocumentGeneration = typeof IdeDocumentGenerationSchema.Type;

export const IdeLanguageGenerationSchema = positiveGeneration("IdeLanguageGeneration");
export type IdeLanguageGeneration = typeof IdeLanguageGenerationSchema.Type;

export const IdeGitSnapshotGenerationSchema = positiveGeneration("IdeGitSnapshotGeneration");
export type IdeGitSnapshotGeneration = typeof IdeGitSnapshotGenerationSchema.Type;

export const IdePlacementGenerationSchema = positiveGeneration("IdePlacementGeneration");
export type IdePlacementGeneration = typeof IdePlacementGenerationSchema.Type;

export const IdePathIndexGenerationSchema = positiveGeneration("IdePathIndexGeneration");
export type IdePathIndexGeneration = typeof IdePathIndexGenerationSchema.Type;

export const IdeServiceGenerationSchema = positiveGeneration("IdeServiceGeneration");
export type IdeServiceGeneration = typeof IdeServiceGenerationSchema.Type;

export const IdeTimestampSchema = Schema.String.pipe(
  Schema.check(
    Schema.isMinLength(20),
    Schema.isMaxLength(35),
    Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u),
  ),
).annotate({ identifier: "IdeTimestamp" });
export type IdeTimestamp = typeof IdeTimestampSchema.Type;

export const IdeTextPositionSchema = Schema.Struct({
  line: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  column: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
}).annotate({ identifier: "IdeTextPosition" });
export type IdeTextPosition = typeof IdeTextPositionSchema.Type;

export const IdeTextRangeSchema = Schema.Struct({
  start: IdeTextPositionSchema,
  end: IdeTextPositionSchema,
}).annotate({ identifier: "IdeTextRange" });
export type IdeTextRange = typeof IdeTextRangeSchema.Type;

export const IdeProjectIdentitySchema = Schema.Struct({
  projectRef: IdeProjectRefSchema,
  rootRef: IdeRootRefSchema,
  worktreeRef: IdeWorktreeRefSchema,
  attachmentRef: IdeAttachmentRefSchema,
  sessionRef: IdeSessionRefSchema,
  grantRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(192)),
}).annotate({ identifier: "IdeProjectIdentity" });
export type IdeProjectIdentity = typeof IdeProjectIdentitySchema.Type;

export const IdeProjectGenerationsSchema = Schema.Struct({
  attachment: IdeAttachmentGenerationSchema,
  pathIndex: IdePathIndexGenerationSchema,
  language: IdeLanguageGenerationSchema,
  gitSnapshot: IdeGitSnapshotGenerationSchema,
  placement: IdePlacementGenerationSchema,
}).annotate({ identifier: "IdeProjectGenerations" });
export type IdeProjectGenerations = typeof IdeProjectGenerationsSchema.Type;

export const IdeFileIdentitySchema = Schema.Struct({
  projectRef: IdeProjectRefSchema,
  rootRef: IdeRootRefSchema,
  worktreeRef: IdeWorktreeRefSchema,
  fileRef: IdeFileRefSchema,
  documentRef: IdeDocumentRefSchema,
  pathRef: DesktopWorkspacePathRefSchema,
}).annotate({ identifier: "IdeFileIdentity" });
export type IdeFileIdentity = typeof IdeFileIdentitySchema.Type;

export const IdeDocumentLifecycleSchema = Schema.TaggedUnion({
  Loading: {
    requestedAt: IdeTimestampSchema,
  },
  Ready: {
    dirty: Schema.Boolean,
    recoverable: Schema.Boolean,
  },
  Conflict: {
    reason: Schema.Literals(["external_change", "external_delete", "stale_save", "grant_revoked"]),
    recoverable: Schema.Boolean,
  },
  Recovering: {
    reason: Schema.Literals(["restart", "model_gap", "service_restart"]),
  },
  Unavailable: {
    reason: Schema.Literals([
      "invalid_ref",
      "missing",
      "directory",
      "binary",
      "too_large",
      "unsupported_encoding",
      "permission_denied",
      "grant_revoked",
      "service_stopped",
    ]),
    recoverable: Schema.Boolean,
  },
}).annotate({ identifier: "IdeDocumentLifecycle" });
export type IdeDocumentLifecycle = typeof IdeDocumentLifecycleSchema.Type;

export const IdeDocumentSnapshotSchema = Schema.Struct({
  identity: IdeFileIdentitySchema,
  attachmentGeneration: IdeAttachmentGenerationSchema,
  documentGeneration: IdeDocumentGenerationSchema,
  diskRevisionRef: Schema.NullOr(IdeDiskRevisionRefSchema),
  encoding: Schema.Literals(["utf-8", "utf-8-bom"]),
  lineEnding: Schema.Literals(["lf", "crlf", "mixed", "none"]),
  lifecycle: IdeDocumentLifecycleSchema,
}).annotate({ identifier: "IdeDocumentSnapshot" });
export type IdeDocumentSnapshot = typeof IdeDocumentSnapshotSchema.Type;

export const IdeEvidenceTierSchema = Schema.Literals([
  "document_local",
  "project_local",
  "owner_managed_remote",
  "managed",
  "unavailable",
]);
export type IdeEvidenceTier = typeof IdeEvidenceTierSchema.Type;

export const IdeCapabilityStateSchema = Schema.TaggedUnion({
  Unconfigured: {},
  Starting: {
    since: IdeTimestampSchema,
    serviceGeneration: IdeServiceGenerationSchema,
  },
  Ready: {
    serviceGeneration: IdeServiceGenerationSchema,
    placementRef: IdePlacementRefSchema,
    evidenceTier: IdeEvidenceTierSchema,
    observedAt: IdeTimestampSchema,
  },
  Degraded: {
    serviceGeneration: IdeServiceGenerationSchema,
    placementRef: IdePlacementRefSchema,
    evidenceTier: IdeEvidenceTierSchema,
    reason: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(400)),
    observedAt: IdeTimestampSchema,
  },
  Stopped: {
    reason: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(400)),
    stoppedAt: IdeTimestampSchema,
  },
  Failed: {
    serviceGeneration: IdeServiceGenerationSchema,
    reason: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(400)),
    retry: Schema.Literals(["none", "manual", "bounded_backoff"]),
    observedAt: IdeTimestampSchema,
  },
}).annotate({ identifier: "IdeCapabilityState" });
export type IdeCapabilityState = typeof IdeCapabilityStateSchema.Type;

export const IdeCapabilityKindSchema = Schema.Literals([
  "path_index",
  "document",
  "language",
  "git",
  "terminal",
  "task",
  "debug",
  "agent",
  "projection",
  "storage",
]);
export type IdeCapabilityKind = typeof IdeCapabilityKindSchema.Type;

export const IdeCapabilitySnapshotSchema = Schema.Struct({
  capabilityRef: IdeCapabilityRefSchema,
  kind: IdeCapabilityKindSchema,
  attachmentGeneration: IdeAttachmentGenerationSchema,
  placementGeneration: IdePlacementGenerationSchema,
  state: IdeCapabilityStateSchema,
}).annotate({ identifier: "IdeCapabilitySnapshot" });
export type IdeCapabilitySnapshot = typeof IdeCapabilitySnapshotSchema.Type;

const navigationBindingFields = {
  navigationRef: IdeNavigationRefSchema,
  projectRef: IdeProjectRefSchema,
  rootRef: IdeRootRefSchema,
  worktreeRef: IdeWorktreeRefSchema,
  attachmentGeneration: IdeAttachmentGenerationSchema,
  origin: Schema.Literals([
    "finder",
    "system_open",
    "explorer",
    "quick_open",
    "search",
    "problems",
    "symbol",
    "git",
    "restore",
    "agent",
    "review",
  ]),
};

export const IdeNavigationTargetSchema = Schema.TaggedUnion({
  File: {
    ...navigationBindingFields,
    fileRef: IdeFileRefSchema,
    documentRef: IdeDocumentRefSchema,
    documentGeneration: IdeDocumentGenerationSchema,
  },
  Range: {
    ...navigationBindingFields,
    fileRef: IdeFileRefSchema,
    documentRef: IdeDocumentRefSchema,
    documentGeneration: IdeDocumentGenerationSchema,
    range: IdeTextRangeSchema,
  },
  Symbol: {
    ...navigationBindingFields,
    symbolRef: IdeSymbolRefSchema,
    languageGeneration: IdeLanguageGenerationSchema,
  },
  Diagnostic: {
    ...navigationBindingFields,
    diagnosticRef: IdeDiagnosticRefSchema,
    languageGeneration: IdeLanguageGenerationSchema,
  },
  Review: {
    ...navigationBindingFields,
    reviewRef: IdeReviewRefSchema,
    gitSnapshotGeneration: IdeGitSnapshotGenerationSchema,
  },
  Proposal: {
    ...navigationBindingFields,
    proposalRef: IdeProposalRefSchema,
  },
}).annotate({ identifier: "IdeNavigationTarget" });
export type IdeNavigationTarget = typeof IdeNavigationTargetSchema.Type;

export const IdeExcerptSchema = Schema.Struct({
  excerptRef: IdeExcerptRefSchema,
  projectRef: IdeProjectRefSchema,
  fileRef: IdeFileRefSchema,
  documentRef: IdeDocumentRefSchema,
  documentGeneration: IdeDocumentGenerationSchema,
  range: IdeTextRangeSchema,
  text: Schema.String.check(Schema.isMaxLength(64_000)),
  truncated: Schema.Boolean,
}).annotate({ identifier: "IdeExcerpt" });
export type IdeExcerpt = typeof IdeExcerptSchema.Type;

export const IdeProposalChangeSchema = Schema.Struct({
  fileRef: IdeFileRefSchema,
  documentRef: IdeDocumentRefSchema,
  baseDiskRevisionRef: Schema.NullOr(IdeDiskRevisionRefSchema),
  baseDocumentGeneration: IdeDocumentGenerationSchema,
  replacementExcerptRef: IdeExcerptRefSchema,
}).annotate({ identifier: "IdeProposalChange" });
export type IdeProposalChange = typeof IdeProposalChangeSchema.Type;

export const IdeProposalSchema = Schema.Struct({
  proposalRef: IdeProposalRefSchema,
  projectRef: IdeProjectRefSchema,
  rootRef: IdeRootRefSchema,
  worktreeRef: IdeWorktreeRefSchema,
  attachmentGeneration: IdeAttachmentGenerationSchema,
  createdAt: IdeTimestampSchema,
  changes: Schema.Array(IdeProposalChangeSchema).check(
    Schema.isMinLength(1),
    Schema.isMaxLength(200),
  ),
}).annotate({ identifier: "IdeProposal" });
export type IdeProposal = typeof IdeProposalSchema.Type;

export const IdeReviewSourceSchema = Schema.TaggedUnion({
  WorkingTree: {
    reviewRef: IdeReviewRefSchema,
    projectRef: IdeProjectRefSchema,
    worktreeRef: IdeWorktreeRefSchema,
    gitSnapshotGeneration: IdeGitSnapshotGenerationSchema,
  },
  Index: {
    reviewRef: IdeReviewRefSchema,
    projectRef: IdeProjectRefSchema,
    worktreeRef: IdeWorktreeRefSchema,
    gitSnapshotGeneration: IdeGitSnapshotGenerationSchema,
  },
  CommitRange: {
    reviewRef: IdeReviewRefSchema,
    projectRef: IdeProjectRefSchema,
    worktreeRef: IdeWorktreeRefSchema,
    baseCommitRef: IdeCommitRefSchema,
    headCommitRef: IdeCommitRefSchema,
    gitSnapshotGeneration: IdeGitSnapshotGenerationSchema,
  },
  Checkpoint: {
    reviewRef: IdeReviewRefSchema,
    projectRef: IdeProjectRefSchema,
    worktreeRef: IdeWorktreeRefSchema,
    checkpointRef: IdeCheckpointRefSchema,
    attachmentGeneration: IdeAttachmentGenerationSchema,
  },
  Proposal: {
    reviewRef: IdeReviewRefSchema,
    projectRef: IdeProjectRefSchema,
    worktreeRef: IdeWorktreeRefSchema,
    proposalRef: IdeProposalRefSchema,
    attachmentGeneration: IdeAttachmentGenerationSchema,
  },
  Conflict: {
    reviewRef: IdeReviewRefSchema,
    projectRef: IdeProjectRefSchema,
    worktreeRef: IdeWorktreeRefSchema,
    documentRef: IdeDocumentRefSchema,
    documentGeneration: IdeDocumentGenerationSchema,
    expectedDiskRevisionRef: Schema.NullOr(IdeDiskRevisionRefSchema),
    actualDiskRevisionRef: Schema.NullOr(IdeDiskRevisionRefSchema),
  },
}).annotate({ identifier: "IdeReviewSource" });
export type IdeReviewSource = typeof IdeReviewSourceSchema.Type;

export const IdeProjectSnapshotSchema = Schema.Struct({
  schemaVersion: IdeProjectSchemaVersion,
  identity: IdeProjectIdentitySchema,
  generations: IdeProjectGenerationsSchema,
  gitSnapshotRef: Schema.NullOr(IdeGitSnapshotRefSchema),
  documents: Schema.Array(IdeDocumentSnapshotSchema).check(Schema.isMaxLength(256)),
  excerpts: Schema.Array(IdeExcerptSchema).check(Schema.isMaxLength(1_000)),
  proposals: Schema.Array(IdeProposalSchema).check(Schema.isMaxLength(200)),
  capabilities: Schema.Array(IdeCapabilitySnapshotSchema).check(Schema.isMaxLength(64)),
  navigation: Schema.Array(IdeNavigationTargetSchema).check(Schema.isMaxLength(200)),
  reviewSources: Schema.Array(IdeReviewSourceSchema).check(Schema.isMaxLength(200)),
  lastEvidenceRef: Schema.NullOr(IdeEvidenceRefSchema),
}).annotate({ identifier: "IdeProjectSnapshot" });
export type IdeProjectSnapshot = typeof IdeProjectSnapshotSchema.Type;

export const IdeGenerationKindSchema = Schema.Literals([
  "attachment",
  "path_index",
  "language",
  "git_snapshot",
  "placement",
]);
export type IdeGenerationKind = typeof IdeGenerationKindSchema.Type;

export const IdeAdvanceGenerationInputSchema = Schema.Struct({
  kind: IdeGenerationKindSchema,
  expectedCurrent: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
}).annotate({ identifier: "IdeAdvanceGenerationInput" });
export type IdeAdvanceGenerationInput = typeof IdeAdvanceGenerationInputSchema.Type;

export const IdeDocumentUpsertInputSchema = Schema.Struct({
  expectedAttachmentGeneration: IdeAttachmentGenerationSchema,
  expectedDocumentGeneration: Schema.NullOr(IdeDocumentGenerationSchema),
  document: IdeDocumentSnapshotSchema,
}).annotate({ identifier: "IdeDocumentUpsertInput" });
export type IdeDocumentUpsertInput = typeof IdeDocumentUpsertInputSchema.Type;

export const IdeCapabilityUpdateInputSchema = Schema.Struct({
  expectedAttachmentGeneration: IdeAttachmentGenerationSchema,
  expectedPlacementGeneration: IdePlacementGenerationSchema,
  capability: IdeCapabilitySnapshotSchema,
}).annotate({ identifier: "IdeCapabilityUpdateInput" });
export type IdeCapabilityUpdateInput = typeof IdeCapabilityUpdateInputSchema.Type;

export const IdeProjectBoundaryDocumentSchema = Schema.Struct({
  schemaVersion: IdeProjectSchemaVersion,
  snapshot: IdeProjectSnapshotSchema,
}).annotate({ identifier: "IdeProjectBoundaryDocument" });
export type IdeProjectBoundaryDocument = typeof IdeProjectBoundaryDocumentSchema.Type;

export const decodeIdeProjectBoundaryDocument = Schema.decodeUnknownEffect(
  IdeProjectBoundaryDocumentSchema,
);
export const decodeIdeProjectSnapshot = Schema.decodeUnknownEffect(IdeProjectSnapshotSchema);
export const decodeIdeNavigationTarget = Schema.decodeUnknownEffect(IdeNavigationTargetSchema);
