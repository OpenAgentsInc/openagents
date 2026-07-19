import { Exit, Schema } from "effect";

import { DesktopWorkspacePathRefSchema } from "../workspace-contract.ts";
import {
  IdeAttachmentGenerationSchema,
  IdeAttachmentRefSchema,
  IdeDiagnosticRefSchema,
  IdeEvidenceTierSchema,
  IdeFileRefSchema,
  IdeLanguageGenerationSchema,
  IdeLanguageServiceRefSchema,
  IdePlacementRefSchema,
  IdeProjectRefSchema,
  IdeRootRefSchema,
  IdeServiceGenerationSchema,
  IdeSymbolRefSchema,
  IdeTimestampSchema,
  IdeWorktreeRefSchema,
} from "./project-contract.ts";
import {
  IdeDocumentGeneration,
  IdeDocumentRef,
  IdeMonacoModelVersion,
} from "./monaco-document-contract.ts";

export const DesktopWorkspaceLanguageRequestChannel =
  "openagents-desktop/workspace-language-request" as const;
export const DesktopWorkspaceLanguageCancelChannel =
  "openagents-desktop/workspace-language-cancel" as const;
export const DesktopWorkspaceLanguageStopChannel =
  "openagents-desktop/workspace-language-stop" as const;

const boundedLanguageRef = <const Identifier extends string>(
  identifier: Identifier,
  prefix: string,
) => Schema.String.pipe(
  Schema.check(
    Schema.isMinLength(prefix.length + 1),
    Schema.isMaxLength(192),
    Schema.isPattern(new RegExp(`^${prefix.replaceAll(".", "\\.")}[A-Za-z0-9][A-Za-z0-9._-]*$`, "u")),
  ),
  Schema.brand(identifier),
).annotate({ identifier });

const boundedCount = (maximum: number) =>
  Schema.Number.check(
    Schema.isInt(),
    Schema.isGreaterThanOrEqualTo(0),
    Schema.isLessThanOrEqualTo(maximum),
  );

export const IdeLanguageRequestRefSchema = boundedLanguageRef(
  "IdeLanguageRequestRef",
  "ide.language-request.",
);
export type IdeLanguageRequestRef = typeof IdeLanguageRequestRefSchema.Type;

export const IdeLanguageResultRefSchema = boundedLanguageRef(
  "IdeLanguageResultRef",
  "ide.language-result.",
);
export type IdeLanguageResultRef = typeof IdeLanguageResultRefSchema.Type;

export const IdeLanguageStartRefSchema = boundedLanguageRef(
  "IdeLanguageStartRef",
  "ide.language-start.",
);
export type IdeLanguageStartRef = typeof IdeLanguageStartRefSchema.Type;

export const IdeLanguageItemRefSchema = boundedLanguageRef(
  "IdeLanguageItemRef",
  "ide.language-item.",
);
export type IdeLanguageItemRef = typeof IdeLanguageItemRefSchema.Type;

export const IdeLanguageCapabilitySchema = Schema.Literals([
  "diagnostics",
  "completion",
  "completion_resolve",
  "hover",
  "definition",
  "declaration",
  "type_definition",
  "references",
  "document_symbols",
  "workspace_symbols",
  "rename_preview",
  "format_document",
  "format_range",
  "code_actions",
  "semantic_tokens",
  "inlay_hints",
  "folding_ranges",
]);
export type IdeLanguageCapability = typeof IdeLanguageCapabilitySchema.Type;

export const IdeLanguageProviderCapabilitySchema = Schema.Struct({
  capability: IdeLanguageCapabilitySchema,
  available: Schema.Boolean,
  reason: Schema.NullOr(
    Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(300)),
  ),
}).annotate({ identifier: "IdeLanguageProviderCapability" });
export type IdeLanguageProviderCapability =
  typeof IdeLanguageProviderCapabilitySchema.Type;

export const IdeLanguageProviderStartSchema = Schema.Struct({
  executable: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(300)),
  providerVersion: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  capabilities: Schema.Array(IdeLanguageProviderCapabilitySchema).check(Schema.isMaxLength(64)),
}).annotate({ identifier: "IdeLanguageProviderStart" });
export type IdeLanguageProviderStart = typeof IdeLanguageProviderStartSchema.Type;

export const IdeLanguagePositionSchema = Schema.Struct({
  line: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  column: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  offset: boundedCount(1_000_000),
}).annotate({ identifier: "IdeLanguagePosition" });
export type IdeLanguagePosition = typeof IdeLanguagePositionSchema.Type;

export const IdeLanguageRangeSchema = Schema.Struct({
  start: IdeLanguagePositionSchema,
  end: IdeLanguagePositionSchema,
}).annotate({ identifier: "IdeLanguageRange" });
export type IdeLanguageRange = typeof IdeLanguageRangeSchema.Type;

export const IdeLanguageRequestSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-language-request.v1"),
  grantRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(192)),
  requestRef: IdeLanguageRequestRefSchema,
  capability: IdeLanguageCapabilitySchema,
  projectRef: IdeProjectRefSchema,
  rootRef: IdeRootRefSchema,
  worktreeRef: IdeWorktreeRefSchema,
  attachmentRef: IdeAttachmentRefSchema,
  attachmentGeneration: IdeAttachmentGenerationSchema,
  languageGeneration: IdeLanguageGenerationSchema,
  documentRef: IdeDocumentRef,
  fileRef: IdeFileRefSchema,
  pathRef: DesktopWorkspacePathRefSchema,
  documentGeneration: IdeDocumentGeneration,
  documentVersion: IdeMonacoModelVersion,
  expectedServiceGeneration: Schema.NullOr(IdeServiceGenerationSchema),
  requestedAt: IdeTimestampSchema,
  language: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  content: Schema.String.check(Schema.isMaxLength(1_000_000)),
  position: Schema.NullOr(IdeLanguagePositionSchema),
  range: Schema.NullOr(IdeLanguageRangeSchema),
  query: Schema.NullOr(Schema.String.check(Schema.isMaxLength(300))),
  limit: Schema.Number.check(
    Schema.isInt(),
    Schema.isGreaterThanOrEqualTo(1),
    Schema.isLessThanOrEqualTo(5_000),
  ),
  timeoutMs: Schema.Number.check(
    Schema.isInt(),
    Schema.isGreaterThanOrEqualTo(50),
    Schema.isLessThanOrEqualTo(30_000),
  ),
}).annotate({ identifier: "IdeLanguageRequest" });
export type IdeLanguageRequest = typeof IdeLanguageRequestSchema.Type;

export const IdeLanguageCancelRequestSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-language-cancel.v1"),
  grantRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(192)),
  requestRef: IdeLanguageRequestRefSchema,
  reason: Schema.Literals(["user", "superseded", "document_replaced", "project_stopped"]),
}).annotate({ identifier: "IdeLanguageCancelRequest" });
export type IdeLanguageCancelRequest = typeof IdeLanguageCancelRequestSchema.Type;

export const IdeLanguageStopRequestSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-language-stop.v1"),
  grantRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(192)),
  reason: Schema.Literals(["project_closed", "project_replaced", "app_quit", "manual_restart"]),
}).annotate({ identifier: "IdeLanguageStopRequest" });
export type IdeLanguageStopRequest = typeof IdeLanguageStopRequestSchema.Type;

const languageItemFields = {
  itemRef: IdeLanguageItemRefSchema,
  resultRef: IdeLanguageResultRefSchema,
  pathRef: DesktopWorkspacePathRefSchema,
  range: Schema.NullOr(IdeLanguageRangeSchema),
};

export const IdeLanguageItemSchema = Schema.TaggedUnion({
  Diagnostic: {
    ...languageItemFields,
    diagnosticRef: IdeDiagnosticRefSchema,
    severity: Schema.Literals(["error", "warning", "information", "hint"]),
    source: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
    code: Schema.NullOr(Schema.String.check(Schema.isMaxLength(80))),
    message: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(2_000)),
  },
  Completion: {
    ...languageItemFields,
    label: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(300)),
    detail: Schema.NullOr(Schema.String.check(Schema.isMaxLength(1_000))),
    kind: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
    insertText: Schema.NullOr(Schema.String.check(Schema.isMaxLength(32_000))),
    sortText: Schema.NullOr(Schema.String.check(Schema.isMaxLength(300))),
  },
  Hover: {
    ...languageItemFields,
    markdown: Schema.String.check(Schema.isMaxLength(64_000)),
  },
  Location: {
    ...languageItemFields,
    relation: Schema.Literals([
      "definition",
      "declaration",
      "type_definition",
      "reference",
    ]),
    preview: Schema.NullOr(Schema.String.check(Schema.isMaxLength(1_000))),
  },
  Symbol: {
    ...languageItemFields,
    symbolRef: IdeSymbolRefSchema,
    name: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(300)),
    kind: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
    containerName: Schema.NullOr(Schema.String.check(Schema.isMaxLength(300))),
    depth: boundedCount(32),
  },
  TextEdit: {
    ...languageItemFields,
    editKind: Schema.Literals(["rename", "format", "code_action"]),
    newText: Schema.String.check(Schema.isMaxLength(1_000_000)),
    expectedDocumentGeneration: IdeDocumentGeneration,
    expectedDocumentVersion: IdeMonacoModelVersion,
  },
  CodeAction: {
    ...languageItemFields,
    title: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(500)),
    actionKind: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
    fixId: Schema.NullOr(Schema.String.check(Schema.isMaxLength(160))),
    editCount: boundedCount(5_000),
  },
  SemanticToken: {
    ...languageItemFields,
    tokenType: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
    modifiers: Schema.Array(
      Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
    ).check(Schema.isMaxLength(32)),
  },
  InlayHint: {
    ...languageItemFields,
    label: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(500)),
    hintKind: Schema.Literals(["type", "parameter", "unknown"]),
    paddingLeft: Schema.Boolean,
    paddingRight: Schema.Boolean,
  },
  FoldingRange: {
    ...languageItemFields,
    foldKind: Schema.Literals(["comment", "imports", "region", "code"]),
  },
}).annotate({ identifier: "IdeLanguageItem" });
export type IdeLanguageItem = typeof IdeLanguageItemSchema.Type;

export const IdeLanguageResultStateSchema = Schema.TaggedUnion({
  Complete: {},
  Partial: {
    reason: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(500)),
  },
  Truncated: {
    limit: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
    omitted: boundedCount(1_000_000),
  },
  Degraded: {
    reason: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(500)),
    recoverable: Schema.Boolean,
  },
  Unavailable: {
    reason: Schema.Literals([
      "unsupported_language",
      "unsupported_capability",
      "missing_provider",
      "incompatible_provider",
      "startup_timeout",
      "provider_failed",
      "malformed_provider_result",
      "grant_revoked",
      "invalid_path",
      "project_stopped",
    ]),
    message: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(800)),
    retry: Schema.Literals(["none", "manual", "bounded_backoff"]),
  },
  Cancelled: {
    reason: Schema.Literals(["user", "superseded", "document_replaced", "project_stopped", "timeout"]),
  },
  Stale: {
    reason: Schema.Literals([
      "attachment_generation_replaced",
      "language_generation_replaced",
      "document_generation_replaced",
      "document_version_replaced",
      "service_generation_replaced",
    ]),
  },
}).annotate({ identifier: "IdeLanguageResultState" });
export type IdeLanguageResultState = typeof IdeLanguageResultStateSchema.Type;

export const IdeLanguageResultSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-language-result.v1"),
  resultRef: IdeLanguageResultRefSchema,
  requestRef: IdeLanguageRequestRefSchema,
  capability: IdeLanguageCapabilitySchema,
  projectRef: IdeProjectRefSchema,
  rootRef: IdeRootRefSchema,
  worktreeRef: IdeWorktreeRefSchema,
  attachmentRef: IdeAttachmentRefSchema,
  attachmentGeneration: IdeAttachmentGenerationSchema,
  languageGeneration: IdeLanguageGenerationSchema,
  documentRef: IdeDocumentRef,
  fileRef: IdeFileRefSchema,
  pathRef: DesktopWorkspacePathRefSchema,
  documentGeneration: IdeDocumentGeneration,
  documentVersion: IdeMonacoModelVersion,
  serviceRef: IdeLanguageServiceRefSchema,
  serviceGeneration: IdeServiceGenerationSchema,
  startRef: IdeLanguageStartRefSchema,
  placementRef: IdePlacementRefSchema,
  evidenceTier: IdeEvidenceTierSchema,
  executable: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(300)),
  providerVersion: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  requestedAt: IdeTimestampSchema,
  observedAt: IdeTimestampSchema,
  freshnessMs: boundedCount(86_400_000),
  state: IdeLanguageResultStateSchema,
  items: Schema.Array(IdeLanguageItemSchema).check(Schema.isMaxLength(5_000)),
  excerpt: Schema.NullOr(Schema.String.check(Schema.isMaxLength(64_000))),
  capabilities: Schema.Array(IdeLanguageProviderCapabilitySchema).check(Schema.isMaxLength(64)),
}).annotate({ identifier: "IdeLanguageResult" });
export type IdeLanguageResult = typeof IdeLanguageResultSchema.Type;

export const IdeLanguageServiceSnapshotSchema = Schema.TaggedUnion({
  Unconfigured: {
    serviceRef: IdeLanguageServiceRefSchema,
  },
  Starting: {
    serviceRef: IdeLanguageServiceRefSchema,
    serviceGeneration: IdeServiceGenerationSchema,
    startRef: IdeLanguageStartRefSchema,
    since: IdeTimestampSchema,
    attempt: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  },
  Ready: {
    serviceRef: IdeLanguageServiceRefSchema,
    serviceGeneration: IdeServiceGenerationSchema,
    startRef: IdeLanguageStartRefSchema,
    placementRef: IdePlacementRefSchema,
    evidenceTier: IdeEvidenceTierSchema,
    executable: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(300)),
    providerVersion: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
    capabilities: Schema.Array(IdeLanguageProviderCapabilitySchema).check(Schema.isMaxLength(64)),
    startedAt: IdeTimestampSchema,
    activeRequests: boundedCount(1_000),
    queuedRequests: boundedCount(1_000),
    restartCount: boundedCount(100),
  },
  Degraded: {
    serviceRef: IdeLanguageServiceRefSchema,
    serviceGeneration: IdeServiceGenerationSchema,
    startRef: IdeLanguageStartRefSchema,
    placementRef: IdePlacementRefSchema,
    evidenceTier: IdeEvidenceTierSchema,
    reason: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(800)),
    recoverable: Schema.Boolean,
    retryAt: Schema.NullOr(IdeTimestampSchema),
    restartCount: boundedCount(100),
  },
  Failed: {
    serviceRef: IdeLanguageServiceRefSchema,
    serviceGeneration: IdeServiceGenerationSchema,
    startRef: IdeLanguageStartRefSchema,
    reason: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(800)),
    retry: Schema.Literals(["none", "manual", "bounded_backoff"]),
    observedAt: IdeTimestampSchema,
    restartCount: boundedCount(100),
  },
  Stopped: {
    serviceRef: IdeLanguageServiceRefSchema,
    reason: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(800)),
    stoppedAt: IdeTimestampSchema,
    activeRequests: Schema.Literal(0),
    queuedRequests: Schema.Literal(0),
  },
}).annotate({ identifier: "IdeLanguageServiceSnapshot" });
export type IdeLanguageServiceSnapshot = typeof IdeLanguageServiceSnapshotSchema.Type;

export const IdeLanguageRejectionReasonSchema = Schema.Literals([
  "invalid_request",
  "stale_generation",
  "provider_unavailable",
  "timeout",
  "malformed_result",
  "queue_full",
  "project_stopped",
]);
export type IdeLanguageRejectionReason = typeof IdeLanguageRejectionReasonSchema.Type;

export const IdeLanguageRequestResponseSchema = Schema.TaggedUnion({
  Result: {
    result: IdeLanguageResultSchema,
    service: IdeLanguageServiceSnapshotSchema,
  },
  Rejected: {
    requestRef: IdeLanguageRequestRefSchema,
    reason: IdeLanguageRejectionReasonSchema,
    message: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(800)),
    service: IdeLanguageServiceSnapshotSchema,
  },
}).annotate({ identifier: "IdeLanguageRequestResponse" });
export type IdeLanguageRequestResponse = typeof IdeLanguageRequestResponseSchema.Type;

export const IdeLanguageCancelResponseSchema = Schema.Struct({
  requestRef: IdeLanguageRequestRefSchema,
  acknowledged: Schema.Boolean,
}).annotate({ identifier: "IdeLanguageCancelResponse" });
export type IdeLanguageCancelResponse = typeof IdeLanguageCancelResponseSchema.Type;

export const IdeLanguageStopResponseSchema = Schema.Struct({
  service: IdeLanguageServiceSnapshotSchema,
}).annotate({ identifier: "IdeLanguageStopResponse" });
export type IdeLanguageStopResponse = typeof IdeLanguageStopResponseSchema.Type;

export const IdeMonacoLocalLanguageStateSchema = Schema.TaggedUnion({
  Unsupported: {
    language: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  },
  Loading: {
    language: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
    workerGeneration: IdeServiceGenerationSchema,
  },
  Ready: {
    language: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
    workerGeneration: IdeServiceGenerationSchema,
    documentRef: IdeDocumentRef,
    documentGeneration: IdeDocumentGeneration,
    documentVersion: IdeMonacoModelVersion,
    evidenceTier: Schema.Literal("document_local"),
    capabilities: Schema.Array(
      Schema.Literals(["syntax", "completion", "hover", "format", "folding"]),
    ).check(Schema.isMaxLength(8)),
  },
  Failed: {
    language: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
    workerGeneration: IdeServiceGenerationSchema,
    message: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(500)),
    recoverable: Schema.Boolean,
  },
}).annotate({ identifier: "IdeMonacoLocalLanguageState" });
export type IdeMonacoLocalLanguageState = typeof IdeMonacoLocalLanguageStateSchema.Type;

export const IdeMonacoProjectLanguageProjectionSchema = Schema.Struct({
  documentRef: IdeDocumentRef,
  documentGeneration: IdeDocumentGeneration,
  documentVersion: IdeMonacoModelVersion,
  serviceGeneration: IdeServiceGenerationSchema,
  evidenceTier: Schema.Literal("project_local"),
  resultRefs: Schema.Array(IdeLanguageResultRefSchema).check(Schema.isMaxLength(16)),
  diagnostics: Schema.Array(Schema.Struct({
    diagnosticRef: IdeDiagnosticRefSchema,
    severity: Schema.Literals(["error", "warning", "information", "hint"]),
    message: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(2_000)),
    source: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
    range: IdeLanguageRangeSchema,
  })).check(Schema.isMaxLength(2_000)),
  semanticTokens: Schema.Array(Schema.Struct({
    itemRef: IdeLanguageItemRefSchema,
    tokenType: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
    range: IdeLanguageRangeSchema,
  })).check(Schema.isMaxLength(5_000)),
  inlayHints: Schema.Array(Schema.Struct({
    itemRef: IdeLanguageItemRefSchema,
    label: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(500)),
    hintKind: Schema.Literals(["type", "parameter", "unknown"]),
    range: IdeLanguageRangeSchema,
  })).check(Schema.isMaxLength(2_000)),
  foldingRanges: Schema.Array(Schema.Struct({
    itemRef: IdeLanguageItemRefSchema,
    foldKind: Schema.Literals(["comment", "imports", "region", "code"]),
    range: IdeLanguageRangeSchema,
  })).check(Schema.isMaxLength(2_000)),
}).annotate({ identifier: "IdeMonacoProjectLanguageProjection" });
export type IdeMonacoProjectLanguageProjection =
  typeof IdeMonacoProjectLanguageProjectionSchema.Type;

export const decodeIdeLanguageRequest = (value: unknown): IdeLanguageRequest | null => {
  const decoded = Schema.decodeUnknownExit(IdeLanguageRequestSchema)(value);
  return Exit.isSuccess(decoded) ? decoded.value : null;
};

export const decodeIdeLanguageCancelRequest = (
  value: unknown,
): IdeLanguageCancelRequest | null => {
  const decoded = Schema.decodeUnknownExit(IdeLanguageCancelRequestSchema)(value);
  return Exit.isSuccess(decoded) ? decoded.value : null;
};

export const decodeIdeLanguageStopRequest = (
  value: unknown,
): IdeLanguageStopRequest | null => {
  const decoded = Schema.decodeUnknownExit(IdeLanguageStopRequestSchema)(value);
  return Exit.isSuccess(decoded) ? decoded.value : null;
};

export const decodeIdeLanguageResult = (value: unknown): IdeLanguageResult | null => {
  const decoded = Schema.decodeUnknownExit(IdeLanguageResultSchema)(value);
  return Exit.isSuccess(decoded) ? decoded.value : null;
};

export const decodeIdeLanguageRequestResponse = (
  value: unknown,
): IdeLanguageRequestResponse | null => {
  const decoded = Schema.decodeUnknownExit(IdeLanguageRequestResponseSchema)(value);
  return Exit.isSuccess(decoded) ? decoded.value : null;
};

export const decodeIdeLanguageCancelResponse = (
  value: unknown,
): IdeLanguageCancelResponse | null => {
  const decoded = Schema.decodeUnknownExit(IdeLanguageCancelResponseSchema)(value);
  return Exit.isSuccess(decoded) ? decoded.value : null;
};

export const decodeIdeLanguageStopResponse = (
  value: unknown,
): IdeLanguageStopResponse | null => {
  const decoded = Schema.decodeUnknownExit(IdeLanguageStopResponseSchema)(value);
  return Exit.isSuccess(decoded) ? decoded.value : null;
};
