import { Exit, Schema } from "effect";

import {
  IdeAttachmentGenerationSchema,
  IdeProjectRefSchema,
  IdeRootRefSchema,
  IdeTimestampSchema,
  IdeWorktreeRefSchema,
} from "./project-contract.ts";
import { IdeRunActorSchema } from "./run-contract.ts";

const boundedText = (maximum: number) => Schema.String.check(Schema.isMaxLength(maximum));
const nonEmptyText = (maximum: number) =>
  Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(maximum));
const boundedCount = (maximum: number) =>
  Schema.Number.check(
    Schema.isInt(),
    Schema.isGreaterThanOrEqualTo(0),
    Schema.isLessThanOrEqualTo(maximum),
  );
const positiveGeneration = <const Identifier extends string>(identifier: Identifier) =>
  Schema.Number.pipe(
    Schema.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
    Schema.brand(identifier),
  ).annotate({ identifier });
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

export const IdeSourceControlSchemaVersion = Schema.Literal(
  "openagents.desktop.ide-source-control.v1",
);
export const IdeSourceControlChannel = "openagents-desktop/ide-source-control" as const;
export const IdeSourceControlSnapshotChannel = "openagents-desktop/ide-source-control-snapshot" as const;

export const IdeRepositoryRefSchema = boundedRef("IdeRepositoryRef", "ide.repository.");
export type IdeRepositoryRef = typeof IdeRepositoryRefSchema.Type;
export const IdeRepositoryGenerationSchema = positiveGeneration("IdeRepositoryGeneration");
export type IdeRepositoryGeneration = typeof IdeRepositoryGenerationSchema.Type;
export const IdeSourceControlRefGenerationSchema = positiveGeneration(
  "IdeSourceControlRefGeneration",
);
export const IdeSourceControlConfigGenerationSchema = positiveGeneration(
  "IdeSourceControlConfigGeneration",
);
export const IdeSourceControlRemoteGenerationSchema = positiveGeneration(
  "IdeSourceControlRemoteGeneration",
);
export const IdeSourceControlCredentialHelperGenerationSchema = positiveGeneration(
  "IdeSourceControlCredentialHelperGeneration",
);
export const IdeSourceControlOperationRefSchema = boundedRef(
  "IdeSourceControlOperationRef",
  "ide.scm-operation.",
);
export type IdeSourceControlOperationRef = typeof IdeSourceControlOperationRefSchema.Type;
export const IdeSourceControlReceiptRefSchema = boundedRef(
  "IdeSourceControlReceiptRef",
  "ide.scm-receipt.",
);
export type IdeSourceControlReceiptRef = typeof IdeSourceControlReceiptRefSchema.Type;
export const IdeSourceControlRecoveryRefSchema = boundedRef(
  "IdeSourceControlRecoveryRef",
  "ide.scm-recovery.",
);
export type IdeSourceControlRecoveryRef = typeof IdeSourceControlRecoveryRefSchema.Type;
export const IdeSourceControlDiffRefSchema = boundedRef("IdeSourceControlDiffRef", "ide.scm-diff.");
export type IdeSourceControlDiffRef = typeof IdeSourceControlDiffRefSchema.Type;
export const IdeSourceControlProviderRefSchema = boundedRef(
  "IdeSourceControlProviderRef",
  "ide.scm-provider.",
);
export type IdeSourceControlProviderRef = typeof IdeSourceControlProviderRefSchema.Type;

export const IdeGitOidSchema = Schema.String.check(
  Schema.isMinLength(40),
  Schema.isMaxLength(64),
  Schema.isPattern(/^[0-9a-f]+$/u),
).annotate({ identifier: "IdeGitOid" });
export type IdeGitOid = typeof IdeGitOidSchema.Type;

export const IdeSourceControlBindingSchema = Schema.Struct({
  projectRef: IdeProjectRefSchema,
  rootRef: IdeRootRefSchema,
  worktreeRef: IdeWorktreeRefSchema,
  attachmentGeneration: IdeAttachmentGenerationSchema,
  repositoryRef: IdeRepositoryRefSchema,
}).annotate({ identifier: "IdeSourceControlBinding" });
export interface IdeSourceControlBinding extends Schema.Schema.Type<
  typeof IdeSourceControlBindingSchema
> {}

export const IdeSourceControlVersionSchema = Schema.Struct({
  repositoryGeneration: IdeRepositoryGenerationSchema,
  statusRef: nonEmptyText(192),
  headOid: Schema.NullOr(IdeGitOidSchema),
  indexOid: nonEmptyText(96),
  worktreeOid: nonEmptyText(96),
  refGeneration: IdeSourceControlRefGenerationSchema,
  configGeneration: IdeSourceControlConfigGenerationSchema,
  remoteGeneration: IdeSourceControlRemoteGenerationSchema,
  credentialHelperGeneration: IdeSourceControlCredentialHelperGenerationSchema,
}).annotate({ identifier: "IdeSourceControlVersion" });
export interface IdeSourceControlVersion extends Schema.Schema.Type<
  typeof IdeSourceControlVersionSchema
> {}

export const IdeSourceControlFileStateSchema = Schema.Literals([
  "unmodified",
  "added",
  "modified",
  "deleted",
  "renamed",
  "copied",
  "type_changed",
  "untracked",
  "ignored",
  "conflicted",
  "submodule",
  "binary",
  "lfs_pointer",
]);
export type IdeSourceControlFileState = typeof IdeSourceControlFileStateSchema.Type;

export const IdeSourceControlConflictStageSchema = Schema.Struct({
  baseOid: Schema.NullOr(IdeGitOidSchema),
  oursOid: Schema.NullOr(IdeGitOidSchema),
  theirsOid: Schema.NullOr(IdeGitOidSchema),
}).annotate({ identifier: "IdeSourceControlConflictStage" });

export const IdeSourceControlPathSchema = Schema.Struct({
  path: nonEmptyText(1_024),
  priorPath: Schema.NullOr(nonEmptyText(1_024)),
  indexState: IdeSourceControlFileStateSchema,
  worktreeState: IdeSourceControlFileStateSchema,
  baseOid: Schema.NullOr(IdeGitOidSchema),
  indexOid: Schema.NullOr(IdeGitOidSchema),
  worktreeOid: Schema.NullOr(nonEmptyText(96)),
  modeBefore: Schema.NullOr(nonEmptyText(12)),
  modeAfter: Schema.NullOr(nonEmptyText(12)),
  conflict: Schema.NullOr(IdeSourceControlConflictStageSchema),
  secretWithheld: Schema.Boolean,
  ignored: Schema.Boolean,
  binary: Schema.Boolean,
  truncated: Schema.Boolean,
  stagedDiffRef: Schema.NullOr(IdeSourceControlDiffRefSchema),
  unstagedDiffRef: Schema.NullOr(IdeSourceControlDiffRefSchema),
}).annotate({ identifier: "IdeSourceControlPath" });
export interface IdeSourceControlPath extends Schema.Schema.Type<
  typeof IdeSourceControlPathSchema
> {}

export const IdeSourceControlOperationStateSchema = Schema.TaggedUnion({
  Idle: {},
  Merge: { headName: Schema.NullOr(nonEmptyText(240)) },
  Rebase: { onto: Schema.NullOr(nonEmptyText(240)), currentStep: boundedCount(100_000) },
  CherryPick: { commitOid: Schema.NullOr(IdeGitOidSchema) },
  Revert: { commitOid: Schema.NullOr(IdeGitOidSchema) },
}).annotate({ identifier: "IdeSourceControlOperationState" });
export type IdeSourceControlOperationState = typeof IdeSourceControlOperationStateSchema.Type;

export const IdeSourceControlDeliveryPhaseSchema = Schema.Literals([
  "changed",
  "reviewed",
  "committed",
  "pushed",
  "pull_request_open",
  "checks_pending",
  "checks_failed",
  "review_requested",
  "review_approved",
  "merged",
  "owner_accepted",
  "released",
]);
export type IdeSourceControlDeliveryPhase = typeof IdeSourceControlDeliveryPhaseSchema.Type;

export const IdeSourceControlDeliveryFactSchema = Schema.Struct({
  phase: IdeSourceControlDeliveryPhaseSchema,
  proven: Schema.Boolean,
  evidenceRefs: Schema.Array(nonEmptyText(320)).check(Schema.isMaxLength(64)),
  observedAt: IdeTimestampSchema,
  freshness: Schema.Literals(["current", "stale", "unknown"]),
}).annotate({ identifier: "IdeSourceControlDeliveryFact" });

export const IdeSourceControlWorktreeSchema = Schema.Struct({
  worktreeRef: IdeWorktreeRefSchema,
  branch: Schema.NullOr(nonEmptyText(240)),
  headOid: Schema.NullOr(IdeGitOidSchema),
  detached: Schema.Boolean,
  locked: Schema.Boolean,
  prunable: Schema.Boolean,
  ownerRef: Schema.NullOr(nonEmptyText(192)),
  activeSessionRef: Schema.NullOr(nonEmptyText(192)),
  removalPreviewRef: Schema.NullOr(nonEmptyText(192)),
  managed: Schema.Boolean,
  dirty: Schema.Boolean,
  changed: Schema.Boolean,
  unpushed: Schema.Boolean,
}).annotate({ identifier: "IdeSourceControlWorktree" });

export const IdeSourceControlSnapshotSchema = Schema.Struct({
  schemaVersion: IdeSourceControlSchemaVersion,
  binding: IdeSourceControlBindingSchema,
  version: IdeSourceControlVersionSchema,
  branch: Schema.NullOr(nonEmptyText(240)),
  upstream: Schema.NullOr(nonEmptyText(320)),
  detached: Schema.Boolean,
  ahead: boundedCount(1_000_000),
  behind: boundedCount(1_000_000),
  operation: IdeSourceControlOperationStateSchema,
  paths: Schema.Array(IdeSourceControlPathSchema).check(Schema.isMaxLength(100_000)),
  worktrees: Schema.Array(IdeSourceControlWorktreeSchema).check(Schema.isMaxLength(10_000)),
  delivery: Schema.Array(IdeSourceControlDeliveryFactSchema).check(Schema.isMaxLength(32)),
  omittedPathCount: boundedCount(10_000_000),
  truncated: Schema.Boolean,
  observedAt: IdeTimestampSchema,
  stopped: Schema.Boolean,
}).annotate({ identifier: "IdeSourceControlSnapshot" });
export interface IdeSourceControlSnapshot extends Schema.Schema.Type<
  typeof IdeSourceControlSnapshotSchema
> {}

const expectedMutation = {
  operationRef: IdeSourceControlOperationRefSchema,
  binding: IdeSourceControlBindingSchema,
  expected: IdeSourceControlVersionSchema,
  actor: IdeRunActorSchema,
  approvalRef: Schema.NullOr(nonEmptyText(192)),
};
const observedCommand = {
  operationRef: IdeSourceControlOperationRefSchema,
  binding: IdeSourceControlBindingSchema,
  actor: IdeRunActorSchema,
  approvalRef: Schema.NullOr(nonEmptyText(192)),
};

export const IdeSourceControlSelectionSchema = Schema.TaggedUnion({
  Paths: { paths: Schema.Array(nonEmptyText(1_024)).check(Schema.isMinLength(1), Schema.isMaxLength(5_000)) },
  Patch: {
    diffRef: IdeSourceControlDiffRefSchema,
    path: nonEmptyText(1_024),
    patch: nonEmptyText(500_000),
    selectedHunks: Schema.Array(boundedCount(100_000)).check(Schema.isMaxLength(10_000)),
    selectedLines: Schema.Array(boundedCount(10_000_000)).check(Schema.isMaxLength(100_000)),
  },
}).annotate({ identifier: "IdeSourceControlSelection" });
export type IdeSourceControlSelection = typeof IdeSourceControlSelectionSchema.Type;

export const IdeSourceControlCommandSchema = Schema.TaggedUnion({
  Refresh: { binding: IdeSourceControlBindingSchema },
  Stage: { ...expectedMutation, selection: IdeSourceControlSelectionSchema },
  Unstage: { ...expectedMutation, selection: IdeSourceControlSelectionSchema },
  Discard: { ...expectedMutation, selection: IdeSourceControlSelectionSchema, recoveryRequired: Schema.Literal(true) },
  Recover: { ...expectedMutation, recoveryRef: IdeSourceControlRecoveryRefSchema },
  Commit: {
    ...expectedMutation,
    message: nonEmptyText(20_000),
    amend: Schema.Boolean,
    sign: Schema.Boolean,
    runHooks: Schema.Boolean,
  },
  BranchCreate: { ...expectedMutation, name: nonEmptyText(240), checkout: Schema.Boolean },
  TagCreate: { ...expectedMutation, name: nonEmptyText(240), targetOid: IdeGitOidSchema, sign: Schema.Boolean },
  Switch: { ...expectedMutation, refName: nonEmptyText(320), detach: Schema.Boolean },
  Merge: { ...expectedMutation, refName: nonEmptyText(320), noFastForward: Schema.Boolean },
  Rebase: { ...expectedMutation, upstream: nonEmptyText(320), onto: Schema.NullOr(nonEmptyText(320)) },
  CherryPick: { ...expectedMutation, commitOids: Schema.Array(IdeGitOidSchema).check(Schema.isMinLength(1), Schema.isMaxLength(1_000)) },
  Revert: { ...expectedMutation, commitOids: Schema.Array(IdeGitOidSchema).check(Schema.isMinLength(1), Schema.isMaxLength(1_000)) },
  Continue: { ...expectedMutation, operation: Schema.Literals(["merge", "rebase", "cherry_pick", "revert"]) },
  Abort: { ...expectedMutation, operation: Schema.Literals(["merge", "rebase", "cherry_pick", "revert"]) },
  Fetch: { ...expectedMutation, remote: nonEmptyText(240), prune: Schema.Boolean },
  Pull: { ...expectedMutation, remote: nonEmptyText(240), branch: nonEmptyText(240), strategy: Schema.Literals(["ff_only", "merge", "rebase"]) },
  Push: {
    ...expectedMutation,
    remote: nonEmptyText(240),
    refspec: nonEmptyText(512),
    forcePolicy: Schema.Literals(["forbid", "force_with_lease"]),
    expectedRemoteOid: Schema.NullOr(IdeGitOidSchema),
  },
  WorktreeCreate: { ...expectedMutation, worktreeRef: IdeWorktreeRefSchema, branch: nonEmptyText(240), ownerRef: Schema.NullOr(nonEmptyText(192)) },
  WorktreeRemove: { ...expectedMutation, worktreeRef: IdeWorktreeRefSchema, previewRef: nonEmptyText(192), recoverable: Schema.Boolean },
  WorktreeRepair: { ...expectedMutation },
  History: { ...observedCommand, commitish: nonEmptyText(320), limit: boundedCount(10_000) },
  Blame: { ...observedCommand, path: nonEmptyText(1_024), commitOid: IdeGitOidSchema },
  ProviderRefresh: { ...observedCommand, providerRef: IdeSourceControlProviderRefSchema },
}).annotate({ identifier: "IdeSourceControlCommand" });
export type IdeSourceControlCommand = typeof IdeSourceControlCommandSchema.Type;

export const IdeSourceControlReceiptSchema = Schema.Struct({
  schemaVersion: IdeSourceControlSchemaVersion,
  receiptRef: IdeSourceControlReceiptRefSchema,
  operationRef: IdeSourceControlOperationRefSchema,
  command: nonEmptyText(80),
  binding: IdeSourceControlBindingSchema,
  preVersion: IdeSourceControlVersionSchema,
  postVersion: IdeSourceControlVersionSchema,
  postImage: IdeSourceControlSnapshotSchema,
  changedPaths: Schema.Array(nonEmptyText(1_024)).check(Schema.isMaxLength(100_000)),
  conflictPaths: Schema.Array(nonEmptyText(1_024)).check(Schema.isMaxLength(100_000)),
  omittedFacts: Schema.Array(nonEmptyText(320)).check(Schema.isMaxLength(1_000)),
  recoveryRef: Schema.NullOr(IdeSourceControlRecoveryRefSchema),
  deliveryFacts: Schema.Array(IdeSourceControlDeliveryFactSchema).check(Schema.isMaxLength(32)),
  actor: IdeRunActorSchema,
  approvalRef: Schema.NullOr(nonEmptyText(192)),
  completedAt: IdeTimestampSchema,
  observation: Schema.NullOr(Schema.TaggedUnion({
    History: {
      commitish: nonEmptyText(320),
      entries: Schema.Array(Schema.Struct({
        commitOid: IdeGitOidSchema,
        parentOids: Schema.Array(IdeGitOidSchema).check(Schema.isMaxLength(64)),
        author: boundedText(320),
        authoredAt: boundedText(80),
        summary: boundedText(2_000),
      })).check(Schema.isMaxLength(10_000)),
      truncated: Schema.Boolean,
    },
    Blame: {
      path: nonEmptyText(1_024),
      commitOid: IdeGitOidSchema,
      lines: Schema.Array(Schema.Struct({
        sourceOid: IdeGitOidSchema,
        originalLine: boundedCount(100_000_000),
        finalLine: boundedCount(100_000_000),
        author: boundedText(320),
        summary: boundedText(2_000),
      })).check(Schema.isMaxLength(100_000)),
      truncated: Schema.Boolean,
    },
    Provider: {
      providerRef: IdeSourceControlProviderRefSchema,
      facts: Schema.Array(Schema.Struct({ key: nonEmptyText(160), value: boundedText(2_000) })).check(Schema.isMaxLength(1_000)),
      freshness: Schema.Literals(["current", "stale", "unknown"]),
    },
  })),
}).annotate({ identifier: "IdeSourceControlReceipt" });
export interface IdeSourceControlReceipt extends Schema.Schema.Type<
  typeof IdeSourceControlReceiptSchema
> {}

export const IdeSourceControlFailureCodeSchema = Schema.Literals([
  "invalid_command",
  "repository_unavailable",
  "stale_version",
  "index_locked",
  "dirty_state",
  "conflict_state",
  "policy_refused",
  "approval_required",
  "credential_unavailable",
  "signing_failed",
  "hook_failed",
  "network_failed",
  "remote_rejected",
  "non_fast_forward",
  "partial_application",
  "submodule_failed",
  "lfs_failed",
  "cancelled",
  "recovery_unavailable",
  "provider_unavailable",
  "operation_failed",
  "stopped",
]);
export type IdeSourceControlFailureCode = typeof IdeSourceControlFailureCodeSchema.Type;

export const IdeSourceControlFailureSchema = Schema.Struct({
  schemaVersion: IdeSourceControlSchemaVersion,
  operationRef: Schema.NullOr(IdeSourceControlOperationRefSchema),
  code: IdeSourceControlFailureCodeSchema,
  message: nonEmptyText(500),
  currentVersion: Schema.NullOr(IdeSourceControlVersionSchema),
  conflictPaths: Schema.Array(nonEmptyText(1_024)).check(Schema.isMaxLength(10_000)),
  recoveryRef: Schema.NullOr(IdeSourceControlRecoveryRefSchema),
  retryable: Schema.Boolean,
}).annotate({ identifier: "IdeSourceControlFailure" });
export interface IdeSourceControlFailure extends Schema.Schema.Type<
  typeof IdeSourceControlFailureSchema
> {}

export const IdeSourceControlCommandResultSchema = Schema.TaggedUnion({
  Success: {
    snapshot: IdeSourceControlSnapshotSchema,
    receipt: Schema.NullOr(IdeSourceControlReceiptSchema),
  },
  Failure: { failure: IdeSourceControlFailureSchema },
}).annotate({ identifier: "IdeSourceControlCommandResult" });
export type IdeSourceControlCommandResult = typeof IdeSourceControlCommandResultSchema.Type;

export const decodeIdeSourceControlCommand = (value: unknown): IdeSourceControlCommand | null => {
  const decoded = Schema.decodeUnknownExit(IdeSourceControlCommandSchema)(value);
  return Exit.isSuccess(decoded) ? decoded.value : null;
};

export const decodeIdeSourceControlSnapshot = (value: unknown): IdeSourceControlSnapshot | null => {
  const decoded = Schema.decodeUnknownExit(IdeSourceControlSnapshotSchema)(value);
  return Exit.isSuccess(decoded) ? decoded.value : null;
};

export const decodeIdeSourceControlCommandResult = (
  value: unknown,
): IdeSourceControlCommandResult | null => {
  const decoded = Schema.decodeUnknownExit(IdeSourceControlCommandResultSchema)(value);
  return Exit.isSuccess(decoded) ? decoded.value : null;
};
