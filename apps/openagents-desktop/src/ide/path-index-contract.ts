import { Schema } from "effect";

import {
  IdeAttachmentGenerationSchema,
  IdeAttachmentRefSchema,
  IdeDocumentGenerationSchema,
  IdeGitSnapshotGenerationSchema,
  IdeLanguageGenerationSchema,
  IdePathIndexGenerationSchema,
  IdeProjectRefSchema,
  IdeRootRefSchema,
  IdeWorktreeRefSchema,
} from "./project-contract.ts";
import {
  DesktopWorkspaceChangeSchema,
  DesktopWorkspacePathRefSchema,
  DesktopWorkspaceTreeEntrySchema,
} from "../workspace-contract.ts";

const boundedRef = <const Identifier extends string>(identifier: Identifier, prefix: string) =>
  Schema.String.pipe(
    Schema.check(
      Schema.isMinLength(prefix.length + 1),
      Schema.isMaxLength(224),
      Schema.isPattern(
        new RegExp(`^${prefix.replaceAll(".", "\\.")}[A-Za-z0-9][A-Za-z0-9._-]*$`, "u"),
      ),
    ),
    Schema.brand(identifier),
  ).annotate({ identifier });

const boundedCount = (maximum: number) =>
  Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 0, maximum }));

export const IdePathIndexSchemaVersion = Schema.Literal("openagents.desktop.ide-path-index.v1");

export const IdePathNodeRefSchema = boundedRef("IdePathNodeRef", "ide.path-node.");
export type IdePathNodeRef = typeof IdePathNodeRefSchema.Type;

export const IdePathScanRefSchema = boundedRef("IdePathScanRef", "ide.path-scan.");
export type IdePathScanRef = typeof IdePathScanRefSchema.Type;

export const IdePathOperationRefSchema = boundedRef("IdePathOperationRef", "ide.path-operation.");
export type IdePathOperationRef = typeof IdePathOperationRefSchema.Type;

export const IdePathIndexIdentitySchema = Schema.Struct({
  projectRef: IdeProjectRefSchema,
  rootRef: IdeRootRefSchema,
  worktreeRef: IdeWorktreeRefSchema,
  attachmentRef: IdeAttachmentRefSchema,
  attachmentGeneration: IdeAttachmentGenerationSchema,
  pathIndexGeneration: IdePathIndexGenerationSchema,
}).annotate({ identifier: "IdePathIndexIdentity" });
export type IdePathIndexIdentity = typeof IdePathIndexIdentitySchema.Type;

export const IdePathIndexProgressSchema = Schema.Struct({
  discoveredDirectories: boundedCount(2_000_000),
  scannedDirectories: boundedCount(2_000_000),
  discoveredNodes: boundedCount(5_000_000),
  admittedNodes: boundedCount(1_000_000),
  pendingDirectories: boundedCount(2_000_000),
  sourceEpoch: boundedCount(Number.MAX_SAFE_INTEGER),
}).annotate({ identifier: "IdePathIndexProgress" });
export type IdePathIndexProgress = typeof IdePathIndexProgressSchema.Type;

export const IdePathIndexStateSchema = Schema.TaggedUnion({
  Scanning: {
    scanRef: IdePathScanRefSchema,
    progress: IdePathIndexProgressSchema,
    reason: Schema.Literals(["initial", "explicit_rescan", "watcher_overflow", "root_refresh"]),
  },
  Partial: {
    scanRef: IdePathScanRefSchema,
    progress: IdePathIndexProgressSchema,
    reason: Schema.Literals(["lazy_directories", "cancelled", "superseded", "source_error"]),
  },
  Truncated: {
    scanRef: IdePathScanRefSchema,
    progress: IdePathIndexProgressSchema,
    limit: boundedCount(1_000_000),
  },
  Degraded: {
    progress: IdePathIndexProgressSchema,
    reason: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(400)),
    retry: Schema.Literals(["refresh", "rescan", "choose_root"]),
  },
  Unavailable: {
    reason: Schema.Literals([
      "grant_revoked",
      "root_unavailable",
      "permission_denied",
      "service_stopped",
      "generation_replaced",
    ]),
    message: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(400)),
  },
  Error: {
    operation: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
    message: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(400)),
    retry: Schema.Literals(["none", "refresh", "rescan"]),
  },
  Empty: {
    sourceEpoch: boundedCount(Number.MAX_SAFE_INTEGER),
  },
  Ready: {
    sourceEpoch: boundedCount(Number.MAX_SAFE_INTEGER),
    nodeCount: boundedCount(1_000_000),
  },
  Stopped: {
    reason: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(400)),
  },
}).annotate({ identifier: "IdePathIndexState" });
export type IdePathIndexState = typeof IdePathIndexStateSchema.Type;

export const IdePathNodePolicySchema = Schema.TaggedUnion({
  Admitted: {},
  Withheld: {
    reason: Schema.Literals([
      "hidden",
      "ignored",
      "secret",
      "binary",
      "symlink",
      "root_boundary",
      "grant_revoked",
    ]),
  },
}).annotate({ identifier: "IdePathNodePolicy" });
export type IdePathNodePolicy = typeof IdePathNodePolicySchema.Type;

export const IdePathNodeLoadStateSchema = Schema.TaggedUnion({
  Leaf: {},
  Unloaded: {},
  Loading: { scanRef: IdePathScanRefSchema },
  Loaded: { sourceEpoch: boundedCount(Number.MAX_SAFE_INTEGER), complete: Schema.Boolean },
  Failed: { message: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(300)) },
}).annotate({ identifier: "IdePathNodeLoadState" });
export type IdePathNodeLoadState = typeof IdePathNodeLoadStateSchema.Type;

export const IdePathNodeBadgeSchema = Schema.TaggedUnion({
  Git: {
    gitSnapshotGeneration: IdeGitSnapshotGenerationSchema,
    state: Schema.Literals(["added", "modified", "deleted", "renamed", "untracked", "ignored"]),
  },
  Diagnostic: {
    languageGeneration: IdeLanguageGenerationSchema,
    severity: Schema.Literals(["error", "warning", "information", "hint"]),
    count: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 1, maximum: 10_000 })),
  },
  Conflict: {
    documentGeneration: IdeDocumentGenerationSchema,
    reason: Schema.Literals(["external_change", "external_delete", "stale_save"]),
  },
  Dirty: {
    documentGeneration: IdeDocumentGenerationSchema,
  },
  Unavailable: {
    attachmentGeneration: IdeAttachmentGenerationSchema,
    reason: Schema.Literals(["missing", "binary", "too_large", "permission_denied", "grant_revoked"]),
  },
}).annotate({ identifier: "IdePathNodeBadge" });
export type IdePathNodeBadge = typeof IdePathNodeBadgeSchema.Type;

export const IdePathPendingOperationSchema = Schema.TaggedUnion({
  Rename: { operationRef: IdePathOperationRefSchema, destinationPathRef: DesktopWorkspacePathRefSchema },
  Move: { operationRef: IdePathOperationRefSchema, destinationPathRef: DesktopWorkspacePathRefSchema },
  Copy: { operationRef: IdePathOperationRefSchema, destinationPathRef: DesktopWorkspacePathRefSchema },
  Delete: { operationRef: IdePathOperationRefSchema },
  Create: { operationRef: IdePathOperationRefSchema },
}).annotate({ identifier: "IdePathPendingOperation" });
export type IdePathPendingOperation = typeof IdePathPendingOperationSchema.Type;

export const IdePathNodeSchema = Schema.Struct({
  nodeRef: IdePathNodeRefSchema,
  parentNodeRef: Schema.NullOr(IdePathNodeRefSchema),
  pathRef: DesktopWorkspacePathRefSchema,
  name: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(512)),
  kind: Schema.Literals(["file", "directory"]),
  revisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
  expandable: Schema.Boolean,
  sizeBytes: Schema.NullOr(Schema.Number.check(Schema.isGreaterThanOrEqualTo(0))),
  policy: IdePathNodePolicySchema,
  loadState: IdePathNodeLoadStateSchema,
  badges: Schema.Array(IdePathNodeBadgeSchema).check(Schema.isMaxLength(16)),
  pending: Schema.NullOr(IdePathPendingOperationSchema),
}).annotate({ identifier: "IdePathNode" });
export type IdePathNode = typeof IdePathNodeSchema.Type;

export const IdePathIndexInteractionSchema = Schema.Struct({
  expandedNodeRefs: Schema.Array(IdePathNodeRefSchema).check(Schema.isMaxLength(20_000)),
  selectedNodeRef: Schema.NullOr(IdePathNodeRefSchema),
  focusedNodeRef: Schema.NullOr(IdePathNodeRefSchema),
  scrollAnchorNodeRef: Schema.NullOr(IdePathNodeRefSchema),
  revealNodeRef: Schema.NullOr(IdePathNodeRefSchema),
  stickyAncestorNodeRefs: Schema.Array(IdePathNodeRefSchema).check(Schema.isMaxLength(256)),
}).annotate({ identifier: "IdePathIndexInteraction" });
export type IdePathIndexInteraction = typeof IdePathIndexInteractionSchema.Type;

export const IdePathIndexFilterSchema = Schema.TaggedUnion({
  None: {},
  Path: {
    query: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(200)),
    matchingNodeRefs: Schema.Array(IdePathNodeRefSchema).check(Schema.isMaxLength(10_000)),
    truncated: Schema.Boolean,
  },
  ExternalSearch: {
    query: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(200)),
    mode: Schema.Literals(["path", "content"]),
    resultPathRefs: Schema.Array(DesktopWorkspacePathRefSchema).check(Schema.isMaxLength(10_000)),
    truncated: Schema.Boolean,
  },
}).annotate({ identifier: "IdePathIndexFilter" });
export type IdePathIndexFilter = typeof IdePathIndexFilterSchema.Type;

export const IdePathIndexResourcesSchema = Schema.Struct({
  nodeCount: boundedCount(1_000_000),
  loadedDirectoryCount: boundedCount(1_000_000),
  pendingDirectoryCount: boundedCount(1_000_000),
  sourceSubscriptionCount: boundedCount(8),
  estimatedBytes: boundedCount(1_000_000_000),
}).annotate({ identifier: "IdePathIndexResources" });
export type IdePathIndexResources = typeof IdePathIndexResourcesSchema.Type;

export const IdePathIndexSnapshotSchema = Schema.Struct({
  schemaVersion: IdePathIndexSchemaVersion,
  identity: IdePathIndexIdentitySchema,
  state: IdePathIndexStateSchema,
  nodes: Schema.Array(IdePathNodeSchema).check(Schema.isMaxLength(1_000_000)),
  interaction: IdePathIndexInteractionSchema,
  filter: IdePathIndexFilterSchema,
  resources: IdePathIndexResourcesSchema,
}).annotate({ identifier: "IdePathIndexSnapshot" });
export type IdePathIndexSnapshot = typeof IdePathIndexSnapshotSchema.Type;

export const IdePathIndexScanRequestSchema = Schema.Struct({
  identity: IdePathIndexIdentitySchema,
  scanRef: IdePathScanRefSchema,
  reason: Schema.Literals(["initial", "explicit_rescan", "watcher_overflow", "root_refresh"]),
  mode: Schema.Literals(["root_and_expanded", "complete"]),
  chunkSize: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 16, maximum: 500 })),
  maximumNodes: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 1, maximum: 1_000_000 })),
}).annotate({ identifier: "IdePathIndexScanRequest" });
export type IdePathIndexScanRequest = typeof IdePathIndexScanRequestSchema.Type;

export const IdePathIndexReconcileRequestSchema = Schema.Struct({
  identity: IdePathIndexIdentitySchema,
  change: DesktopWorkspaceChangeSchema,
  scanRef: IdePathScanRefSchema,
}).annotate({ identifier: "IdePathIndexReconcileRequest" });
export type IdePathIndexReconcileRequest = typeof IdePathIndexReconcileRequestSchema.Type;

export const IdePathIndexInteractionUpdateSchema = Schema.TaggedUnion({
  Expand: { nodeRef: IdePathNodeRefSchema },
  Collapse: { nodeRef: IdePathNodeRefSchema },
  Select: { nodeRef: Schema.NullOr(IdePathNodeRefSchema) },
  Focus: { nodeRef: Schema.NullOr(IdePathNodeRefSchema) },
  ScrollAnchor: { nodeRef: Schema.NullOr(IdePathNodeRefSchema) },
  Reveal: { nodeRef: IdePathNodeRefSchema },
  Restore: { interaction: IdePathIndexInteractionSchema },
}).annotate({ identifier: "IdePathIndexInteractionUpdate" });
export type IdePathIndexInteractionUpdate = typeof IdePathIndexInteractionUpdateSchema.Type;

export const IdePathIndexBadgeUpdateSchema = Schema.Struct({
  identity: IdePathIndexIdentitySchema,
  gitSnapshotGeneration: IdeGitSnapshotGenerationSchema,
  languageGeneration: IdeLanguageGenerationSchema,
  updates: Schema.Array(Schema.Struct({
    nodeRef: IdePathNodeRefSchema,
    badges: Schema.Array(IdePathNodeBadgeSchema).check(Schema.isMaxLength(16)),
  })).check(Schema.isMaxLength(20_000)),
}).annotate({ identifier: "IdePathIndexBadgeUpdate" });
export type IdePathIndexBadgeUpdate = typeof IdePathIndexBadgeUpdateSchema.Type;

export const IdeExplorerCommandSchema = Schema.TaggedUnion({
  Open: { nodeRef: IdePathNodeRefSchema, pathRef: DesktopWorkspacePathRefSchema },
  Reveal: { nodeRef: IdePathNodeRefSchema, pathRef: DesktopWorkspacePathRefSchema },
  CreateFile: { parentNodeRef: Schema.NullOr(IdePathNodeRefSchema), parentPathRef: DesktopWorkspacePathRefSchema, name: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)) },
  CreateFolder: { parentNodeRef: Schema.NullOr(IdePathNodeRefSchema), parentPathRef: DesktopWorkspacePathRefSchema, name: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)) },
  Rename: { nodeRef: IdePathNodeRefSchema, pathRef: DesktopWorkspacePathRefSchema, expectedRevisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)), name: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)) },
  Move: { nodeRef: IdePathNodeRefSchema, pathRef: DesktopWorkspacePathRefSchema, expectedRevisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)), destinationParentPathRef: DesktopWorkspacePathRefSchema },
  Copy: { nodeRef: IdePathNodeRefSchema, pathRef: DesktopWorkspacePathRefSchema, expectedRevisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)), destinationParentPathRef: DesktopWorkspacePathRefSchema },
  Duplicate: { nodeRef: IdePathNodeRefSchema, pathRef: DesktopWorkspacePathRefSchema, expectedRevisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)) },
  Delete: { nodeRef: IdePathNodeRefSchema, pathRef: DesktopWorkspacePathRefSchema, expectedRevisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)), recursive: Schema.Boolean },
  OpenTerminal: { nodeRef: IdePathNodeRefSchema, pathRef: DesktopWorkspacePathRefSchema },
  Compare: { nodeRef: IdePathNodeRefSchema, pathRef: DesktopWorkspacePathRefSchema },
  Refresh: {},
  Retry: {},
  Rescan: {},
}).annotate({ identifier: "IdeExplorerCommand" });
export type IdeExplorerCommand = typeof IdeExplorerCommandSchema.Type;

export const IdePathIndexOperationUpdateSchema = Schema.TaggedUnion({
  Pending: {
    identity: IdePathIndexIdentitySchema,
    operationRef: IdePathOperationRefSchema,
    command: IdeExplorerCommandSchema,
  },
  Confirmed: {
    identity: IdePathIndexIdentitySchema,
    operationRef: IdePathOperationRefSchema,
    sourceNodeRef: Schema.NullOr(IdePathNodeRefSchema),
    entry: Schema.NullOr(DesktopWorkspaceTreeEntrySchema),
  },
  Refused: {
    identity: IdePathIndexIdentitySchema,
    operationRef: IdePathOperationRefSchema,
    reason: Schema.Literals(["collision", "stale_revision", "permission_denied", "grant_revoked", "unavailable"]),
    message: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(400)),
  },
}).annotate({ identifier: "IdePathIndexOperationUpdate" });
export type IdePathIndexOperationUpdate = typeof IdePathIndexOperationUpdateSchema.Type;

export const IdePierreTreeNodeSchema = Schema.Struct({
  nodeRef: IdePathNodeRefSchema,
  pathRef: DesktopWorkspacePathRefSchema,
  kind: Schema.Literals(["file", "directory"]),
  revisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
  badgeLabels: Schema.Array(Schema.String.check(Schema.isMaxLength(80))).check(Schema.isMaxLength(16)),
  pendingLabel: Schema.NullOr(Schema.String.check(Schema.isMaxLength(160))),
}).annotate({ identifier: "IdePierreTreeNode" });
export type IdePierreTreeNode = typeof IdePierreTreeNodeSchema.Type;

export const IdePierreTreeProjectionSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.pierre-tree-projection.v1"),
  indexGeneration: IdePathIndexGenerationSchema,
  state: IdePathIndexStateSchema,
  nodes: Schema.Array(IdePierreTreeNodeSchema).check(Schema.isMaxLength(50_000)),
  expandedNodeRefs: Schema.Array(IdePathNodeRefSchema).check(Schema.isMaxLength(20_000)),
  selectedNodeRef: Schema.NullOr(IdePathNodeRefSchema),
  focusedNodeRef: Schema.NullOr(IdePathNodeRefSchema),
  scrollAnchorNodeRef: Schema.NullOr(IdePathNodeRefSchema),
  stickyAncestorNodeRefs: Schema.Array(IdePathNodeRefSchema).check(Schema.isMaxLength(256)),
  truncated: Schema.Boolean,
}).annotate({ identifier: "IdePierreTreeProjection" });
export type IdePierreTreeProjection = typeof IdePierreTreeProjectionSchema.Type;

export const decodeIdePathIndexSnapshot = Schema.decodeUnknownEffect(IdePathIndexSnapshotSchema);
export const decodeIdeExplorerCommand = Schema.decodeUnknownEffect(IdeExplorerCommandSchema);
export const decodeIdePierreTreeProjection = Schema.decodeUnknownEffect(IdePierreTreeProjectionSchema);
