import { Exit, Schema } from "effect";

import { DesktopWorkspacePathRefSchema } from "../workspace-contract.ts";
import {
  IdeAgentAttachmentSchema,
  IdeAgentManifestRefSchema,
  IdeAgentProposalBaseSchema,
  IdeAgentProposalSchema,
  IdeAgentTurnRefSchema,
} from "./agent-code-contract.ts";
import {
  IdeDocumentGeneration as IdeMonacoDocumentGeneration,
  IdeDocumentRef as IdeMonacoDocumentRef,
  IdeDocumentSequence,
  IdeMonacoModelVersion,
} from "./monaco-document-contract.ts";
import {
  IdeAttachmentGenerationSchema,
  IdeAttachmentRefSchema,
  IdeDocumentGenerationSchema,
  IdeDocumentRefSchema,
  IdeFileRefSchema,
  IdePlacementGenerationSchema,
  IdePlacementRefSchema,
  IdeProjectRefSchema,
  IdeProposalRefSchema,
  IdeRootRefSchema,
  IdeSessionRefSchema,
  IdeTextRangeSchema,
  IdeTimestampSchema,
  IdeWorktreeRefSchema,
} from "./project-contract.ts";

const bounded = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(512));
const detail = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(2_000));
const nonNegative = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0));
const positive = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1));
const sha256 = Schema.String.check(Schema.isPattern(/^sha256:[0-9a-f]{64}$/u));

const ref = <const Identifier extends string>(identifier: Identifier, prefix: string) =>
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

export const IdeCursorSchemaVersion = Schema.Literal("openagents.ide-cursor.v1");
export const IdeCursorRequestRefSchema = ref("IdeCursorRequestRef", "ide.cursor-request.");
export const IdeCursorCandidateRefSchema = ref("IdeCursorCandidateRef", "ide.cursor-candidate.");
export const IdeCursorDecisionRefSchema = ref("IdeCursorDecisionRef", "ide.cursor-decision.");
export const IdeCursorAttemptRefSchema = ref("IdeCursorAttemptRef", "ide.cursor-attempt.");
export const IdeCursorContextRefSchema = ref("IdeCursorContextRef", "ide.cursor-context.");
export const IdeCursorSequenceSchema = positive
  .pipe(Schema.brand("IdeCursorSequence"))
  .annotate({ identifier: "IdeCursorSequence" });
export const IdeCursorSelectionVersionSchema = nonNegative
  .pipe(Schema.brand("IdeCursorSelectionVersion"))
  .annotate({ identifier: "IdeCursorSelectionVersion" });

export type IdeCursorRequestRef = typeof IdeCursorRequestRefSchema.Type;
export type IdeCursorCandidateRef = typeof IdeCursorCandidateRefSchema.Type;
export type IdeCursorDecisionRef = typeof IdeCursorDecisionRefSchema.Type;
export type IdeCursorSequence = typeof IdeCursorSequenceSchema.Type;

export const IdeCursorEvidenceSchema = Schema.TaggedUnion({
  Observed: { evidenceRef: bounded, observedAt: IdeTimestampSchema },
  ProviderDeclared: { evidenceRef: bounded, observedAt: IdeTimestampSchema },
  RequestedOnly: { reason: detail },
  NotAvailable: { reason: detail },
}).annotate({ identifier: "IdeCursorEvidence" });
export type IdeCursorEvidence = typeof IdeCursorEvidenceSchema.Type;

const identityValue = Schema.Struct({ value: bounded, evidence: IdeCursorEvidenceSchema });
export const IdeCursorExecutionIdentitySchema = Schema.Struct({
  harness: identityValue,
  provider: identityValue,
  model: identityValue,
  account: identityValue,
  placementRef: IdePlacementRefSchema,
  placementGeneration: IdePlacementGenerationSchema,
  indexPosture: Schema.Literals(["disabled", "local", "remote"]),
  networkPosture: Schema.Literals(["offline", "restricted", "networked"]),
}).annotate({ identifier: "IdeCursorExecutionIdentity" });
export type IdeCursorExecutionIdentity = typeof IdeCursorExecutionIdentitySchema.Type;

export const IdeCursorIdentityProgressSchema = Schema.Struct({
  requested: IdeCursorExecutionIdentitySchema,
  admitted: IdeCursorExecutionIdentitySchema,
  effective: IdeCursorExecutionIdentitySchema,
  substitution: Schema.TaggedUnion({
    None: {},
    BeforeContent: { from: bounded, to: bounded, reason: detail },
    NewAttempt: { fromAttemptRef: IdeCursorAttemptRefSchema, reason: detail },
  }),
}).annotate({ identifier: "IdeCursorIdentityProgress" });
export type IdeCursorIdentityProgress = typeof IdeCursorIdentityProgressSchema.Type;

export const IdeCursorMeasuredSchema = Schema.TaggedUnion({
  Measured: {
    value: nonNegative,
    unit: Schema.Literals(["milliseconds", "bytes", "tokens", "count", "usd_micros"]),
  },
  NotMeasured: { reason: detail },
}).annotate({ identifier: "IdeCursorMeasured" });

export const IdeCursorDisclosureSchema = Schema.Struct({
  dataDestinations: Schema.Array(
    Schema.Struct({
      destination: bounded,
      purpose: bounded,
      bytes: IdeCursorMeasuredSchema,
      retention: Schema.Literals(["none", "request", "provider_policy", "unknown"]),
    }),
  ).check(Schema.isMaxLength(32)),
  usage: Schema.Struct({
    input: IdeCursorMeasuredSchema,
    output: IdeCursorMeasuredSchema,
    cost: IdeCursorMeasuredSchema,
  }),
  noRemoteIndexDependency: Schema.Boolean,
  secretsSent: Schema.Literal(false),
}).annotate({ identifier: "IdeCursorDisclosure" });
export type IdeCursorDisclosure = typeof IdeCursorDisclosureSchema.Type;

export const IdeCursorAnchorSchema = Schema.Struct({
  projectRef: IdeProjectRefSchema,
  rootRef: IdeRootRefSchema,
  worktreeRef: IdeWorktreeRefSchema,
  attachmentRef: IdeAttachmentRefSchema,
  attachmentGeneration: IdeAttachmentGenerationSchema,
  sessionRef: IdeSessionRefSchema,
  sourceDocumentRef: IdeMonacoDocumentRef,
  sourceDocumentGeneration: IdeMonacoDocumentGeneration,
  fileRef: IdeFileRefSchema,
  documentRef: IdeDocumentRefSchema,
  documentGeneration: IdeDocumentGenerationSchema,
  documentSequence: IdeDocumentSequence,
  modelVersion: IdeMonacoModelVersion,
  selectionVersion: IdeCursorSelectionVersionSchema,
  pathRef: DesktopWorkspacePathRefSchema,
  selection: IdeTextRangeSchema,
  contentDigest: sha256,
}).annotate({ identifier: "IdeCursorAnchor" });
export type IdeCursorAnchor = typeof IdeCursorAnchorSchema.Type;

export const IdeCursorIntentSchema = Schema.TaggedUnion({
  Complete: { acceptance: Schema.Literals(["word", "line", "all"]) },
  NextEdit: {},
  Ask: { question: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(8_000)) },
  Edit: { instruction: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(8_000)) },
  Generate: { instruction: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(8_000)) },
}).annotate({ identifier: "IdeCursorIntent" });

export const IdeCursorRequestSchema = Schema.Struct({
  schemaVersion: IdeCursorSchemaVersion,
  requestRef: IdeCursorRequestRefSchema,
  attemptRef: IdeCursorAttemptRefSchema,
  sequence: IdeCursorSequenceSchema,
  requestedAt: IdeTimestampSchema,
  anchor: IdeCursorAnchorSchema,
  intent: IdeCursorIntentSchema,
  identity: IdeCursorIdentityProgressSchema,
  contextRef: IdeCursorContextRefSchema,
  contextDigest: sha256,
  budget: Schema.Struct({
    maxLatencyMs: positive,
    maxInputTokens: positive,
    maxOutputTokens: positive,
  }),
}).annotate({ identifier: "IdeCursorRequest" });
export type IdeCursorRequest = typeof IdeCursorRequestSchema.Type;

const candidateFields = {
  schemaVersion: IdeCursorSchemaVersion,
  candidateRef: IdeCursorCandidateRefSchema,
  requestRef: IdeCursorRequestRefSchema,
  attemptRef: IdeCursorAttemptRefSchema,
  sequence: IdeCursorSequenceSchema,
  anchor: IdeCursorAnchorSchema,
  identity: IdeCursorIdentityProgressSchema,
  disclosure: IdeCursorDisclosureSchema,
  provenance: Schema.Array(
    Schema.Struct({
      sourceRef: bounded,
      source: Schema.Literals([
        "document",
        "selection",
        "diagnostic",
        "symbol",
        "history",
        "context",
      ]),
      freshness: Schema.Literals(["current", "stale", "unknown"]),
    }),
  ).check(Schema.isMinLength(1), Schema.isMaxLength(128)),
  quality: Schema.Struct({
    confidence: Schema.Number.check(
      Schema.isGreaterThanOrEqualTo(0),
      Schema.isLessThanOrEqualTo(1),
    ),
    syntaxChecked: Schema.Boolean,
    diagnosticsChecked: Schema.Boolean,
  }),
  staleness: Schema.TaggedUnion({
    Fresh: {},
    Stale: {
      reason: Schema.Literals([
        "document",
        "selection",
        "model",
        "sequence",
        "attachment",
        "proposal",
        "cancelled",
      ]),
    },
  }),
  createdAt: IdeTimestampSchema,
  resultDigest: sha256,
};

export const IdeCursorCandidateSchema = Schema.TaggedUnion({
  Completion: {
    ...candidateFields,
    replace: IdeTextRangeSchema,
    text: Schema.String.check(Schema.isMaxLength(100_000)),
  },
  NextEdit: {
    ...candidateFields,
    targetPathRef: DesktopWorkspacePathRefSchema,
    replace: IdeTextRangeSchema,
    text: Schema.String.check(Schema.isMaxLength(100_000)),
    explanation: detail,
  },
  Answer: { ...candidateFields, markdown: Schema.String.check(Schema.isMaxLength(100_000)) },
  Proposal: {
    ...candidateFields,
    proposalRef: IdeProposalRefSchema,
    proposal: IdeAgentProposalSchema,
  },
}).annotate({ identifier: "IdeCursorCandidate" });
export type IdeCursorCandidate = typeof IdeCursorCandidateSchema.Type;

const cursorFailureFields = {
  requestRef: IdeCursorRequestRefSchema,
  attemptRef: IdeCursorAttemptRefSchema,
  reason: Schema.Literals([
    "cancelled",
    "stale",
    "provider",
    "budget",
    "invalid_output",
    "unavailable",
  ]),
  detail,
};

export const IdeCursorFailureSchema = Schema.Struct(cursorFailureFields).annotate({
  identifier: "IdeCursorFailure",
});
export type IdeCursorFailure = typeof IdeCursorFailureSchema.Type;

export const IdeCursorStreamEventSchema = Schema.TaggedUnion({
  Identity: {
    requestRef: IdeCursorRequestRefSchema,
    attemptRef: IdeCursorAttemptRefSchema,
    identity: IdeCursorIdentityProgressSchema,
  },
  Candidate: { candidate: IdeCursorCandidateSchema },
  Failed: cursorFailureFields,
  Finished: {
    requestRef: IdeCursorRequestRefSchema,
    attemptRef: IdeCursorAttemptRefSchema,
    disclosure: IdeCursorDisclosureSchema,
  },
}).annotate({ identifier: "IdeCursorStreamEvent" });
export type IdeCursorStreamEvent = typeof IdeCursorStreamEventSchema.Type;

export const IdeCursorDecisionSchema = Schema.TaggedUnion({
  Accept: {
    decisionRef: IdeCursorDecisionRefSchema,
    candidateRef: IdeCursorCandidateRefSchema,
    requestRef: IdeCursorRequestRefSchema,
    sequence: IdeCursorSequenceSchema,
    acceptedAt: IdeTimestampSchema,
    granularity: Schema.Literals(["word", "line", "all"]),
    resultDigest: sha256,
  },
  Reject: {
    decisionRef: IdeCursorDecisionRefSchema,
    candidateRef: IdeCursorCandidateRefSchema,
    requestRef: IdeCursorRequestRefSchema,
    sequence: IdeCursorSequenceSchema,
    decidedAt: IdeTimestampSchema,
    reason: detail,
  },
  Compare: {
    decisionRef: IdeCursorDecisionRefSchema,
    candidateRef: IdeCursorCandidateRefSchema,
    requestRef: IdeCursorRequestRefSchema,
    sequence: IdeCursorSequenceSchema,
    decidedAt: IdeTimestampSchema,
  },
  KeepBoth: {
    decisionRef: IdeCursorDecisionRefSchema,
    candidateRef: IdeCursorCandidateRefSchema,
    requestRef: IdeCursorRequestRefSchema,
    sequence: IdeCursorSequenceSchema,
    decidedAt: IdeTimestampSchema,
  },
  Retry: {
    decisionRef: IdeCursorDecisionRefSchema,
    candidateRef: IdeCursorCandidateRefSchema,
    requestRef: IdeCursorRequestRefSchema,
    sequence: IdeCursorSequenceSchema,
    decidedAt: IdeTimestampSchema,
  },
  Cancel: {
    decisionRef: IdeCursorDecisionRefSchema,
    candidateRef: Schema.NullOr(IdeCursorCandidateRefSchema),
    requestRef: IdeCursorRequestRefSchema,
    sequence: IdeCursorSequenceSchema,
    decidedAt: IdeTimestampSchema,
    reason: detail,
  },
  Undo: {
    decisionRef: IdeCursorDecisionRefSchema,
    candidateRef: IdeCursorCandidateRefSchema,
    requestRef: IdeCursorRequestRefSchema,
    sequence: IdeCursorSequenceSchema,
    decidedAt: IdeTimestampSchema,
    resultDigest: sha256,
  },
}).annotate({ identifier: "IdeCursorDecision" });
export type IdeCursorDecision = typeof IdeCursorDecisionSchema.Type;

export const IdeCursorDecisionReceiptSchema = Schema.Struct({
  schemaVersion: IdeCursorSchemaVersion,
  decision: IdeCursorDecisionSchema,
  recordedAt: IdeTimestampSchema,
  previousContentDigest: Schema.NullOr(sha256),
  resultContentDigest: Schema.NullOr(sha256),
  proposalRef: Schema.NullOr(IdeProposalRefSchema),
  proposalSubmitted: Schema.Boolean,
  applied: Schema.Boolean,
  staleRejected: Schema.Boolean,
}).annotate({ identifier: "IdeCursorDecisionReceipt" });
export type IdeCursorDecisionReceipt = typeof IdeCursorDecisionReceiptSchema.Type;

export const IdeCursorCapabilitiesSchema = Schema.Struct({
  providerRef: bounded,
  modelRefs: Schema.Array(bounded).check(Schema.isMinLength(1), Schema.isMaxLength(128)),
  intents: Schema.Array(Schema.Literals(["complete", "next_edit", "ask", "change"])).check(
    Schema.isMinLength(1),
  ),
  noFilesystemAccess: Schema.Literal(true),
  noShellAccess: Schema.Literal(true),
  identityBeforeCandidate: Schema.Literal(true),
  supportsCancellation: Schema.Literal(true),
  supportsOffline: Schema.Boolean,
}).annotate({ identifier: "IdeCursorCapabilities" });
export type IdeCursorCapabilities = typeof IdeCursorCapabilitiesSchema.Type;

export const IdeCursorProviderInputSchema = Schema.Struct({
  request: IdeCursorRequestSchema,
  proposalContext: Schema.Struct({
    attachment: IdeAgentAttachmentSchema,
    manifestRef: IdeAgentManifestRefSchema,
    turnRef: IdeAgentTurnRefSchema,
    conversationThreadRef: Schema.NullOr(bounded),
    bases: Schema.Array(Schema.Struct({
      fileRef: IdeFileRefSchema,
      pathRef: DesktopWorkspacePathRefSchema,
      base: IdeAgentProposalBaseSchema,
    })).check(Schema.isMinLength(1), Schema.isMaxLength(128)),
  }),
  documentText: Schema.String.check(Schema.isMaxLength(1_000_000)),
  context: Schema.Array(
    Schema.Struct({
      contextRef: IdeCursorContextRefSchema,
      source: Schema.Literals(["selection", "diagnostic", "symbol", "history", "workspace"]),
      text: Schema.String.check(Schema.isMaxLength(100_000)),
      contentDigest: sha256,
      freshness: Schema.Literals(["current", "stale"]),
      sensitivity: Schema.Literals(["public", "workspace"]),
    }),
  ).check(Schema.isMaxLength(128)),
}).annotate({ identifier: "IdeCursorProviderInput" });
export type IdeCursorProviderInput = typeof IdeCursorProviderInputSchema.Type;

export const IdeCursorSnapshotSchema = Schema.Struct({
  schemaVersion: IdeCursorSchemaVersion,
  latestSequence: nonNegative,
  activeRequestRef: Schema.NullOr(IdeCursorRequestRefSchema),
  activeAttemptRef: Schema.NullOr(IdeCursorAttemptRefSchema),
  candidates: Schema.Array(IdeCursorCandidateSchema).check(Schema.isMaxLength(32)),
  decisions: Schema.Array(IdeCursorDecisionSchema).check(Schema.isMaxLength(500)),
  receipts: Schema.Array(IdeCursorDecisionReceiptSchema).check(Schema.isMaxLength(500)),
  finalDisclosure: Schema.NullOr(IdeCursorDisclosureSchema),
  failure: Schema.NullOr(IdeCursorFailureSchema),
  state: Schema.Literals(["idle", "running", "complete", "failed", "stopped"]),
}).annotate({ identifier: "IdeCursorSnapshot" });
export type IdeCursorSnapshot = typeof IdeCursorSnapshotSchema.Type;

export const DesktopIdeCursorSnapshotChannel = "openagents-desktop/ide-cursor-snapshot";
export const DesktopIdeCursorCommandChannel = "openagents-desktop/ide-cursor-command";

export const IdeCursorCommandSchema = Schema.TaggedUnion({
  Start: { input: IdeCursorProviderInputSchema },
  Decide: { decision: IdeCursorDecisionSchema },
  Stop: { reason: detail },
}).annotate({ identifier: "IdeCursorCommand" });
export type IdeCursorCommand = typeof IdeCursorCommandSchema.Type;

export const IdeCursorCommandResultSchema = Schema.TaggedUnion({
  Succeeded: { snapshot: IdeCursorSnapshotSchema },
  Refused: {
    reason: Schema.Literals([
      "invalid_input",
      "stale_sequence",
      "stale_anchor",
      "identity_mismatch",
      "candidate_missing",
      "stopped",
      "authority_stale",
      "authority_unavailable",
      "conflict",
      "unavailable",
    ]),
    message: detail,
    snapshot: IdeCursorSnapshotSchema,
  },
}).annotate({ identifier: "IdeCursorCommandResult" });
export type IdeCursorCommandResult = typeof IdeCursorCommandResultSchema.Type;

export const emptyIdeCursorSnapshot = (): IdeCursorSnapshot => ({
  schemaVersion: "openagents.ide-cursor.v1",
  latestSequence: 0,
  activeRequestRef: null,
  activeAttemptRef: null,
  candidates: [],
  decisions: [],
  receipts: [],
  finalDisclosure: null,
  failure: null,
  state: "idle",
});

const decode = <S extends Schema.ConstraintDecoder<unknown, never>>(
  schema: S,
  value: unknown,
): S["Type"] | null => {
  const result = Schema.decodeUnknownExit(schema)(value);
  return Exit.isSuccess(result) ? result.value : null;
};

export const decodeIdeCursorRequest = (value: unknown): IdeCursorRequest | null =>
  decode(IdeCursorRequestSchema, value);
export const decodeIdeCursorCandidate = (value: unknown): IdeCursorCandidate | null =>
  decode(IdeCursorCandidateSchema, value);
export const decodeIdeCursorDecision = (value: unknown): IdeCursorDecision | null =>
  decode(IdeCursorDecisionSchema, value);
export const decodeIdeCursorSnapshot = (value: unknown): IdeCursorSnapshot | null =>
  decode(IdeCursorSnapshotSchema, value);
export const decodeIdeCursorCommand = (value: unknown): IdeCursorCommand | null =>
  decode(IdeCursorCommandSchema, value);
export const decodeIdeCursorCommandResult = (value: unknown): IdeCursorCommandResult | null =>
  decode(IdeCursorCommandResultSchema, value);
