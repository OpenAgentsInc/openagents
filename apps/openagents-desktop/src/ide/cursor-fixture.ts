import { createHash } from "node:crypto";

import { ideAgentFixtureBase, ideAgentFixtureDocument, ideAgentFixtureManifest } from "./agent-code-fixture.ts";

import {
  IdeCursorAnchorSchema,
  IdeCursorAttemptRefSchema,
  IdeCursorCandidateRefSchema,
  IdeCursorCandidateSchema,
  IdeCursorCapabilitiesSchema,
  IdeCursorContextRefSchema,
  IdeCursorDecisionRefSchema,
  IdeCursorDecisionSchema,
  IdeCursorDisclosureSchema,
  IdeCursorExecutionIdentitySchema,
  IdeCursorIdentityProgressSchema,
  IdeCursorSelectionVersionSchema,
  IdeCursorProviderInputSchema,
  IdeCursorRequestRefSchema,
  IdeCursorRequestSchema,
  IdeCursorSequenceSchema,
  type IdeCursorAnchor,
  type IdeCursorCandidate,
  type IdeCursorDecision,
  type IdeCursorIdentityProgress,
  type IdeCursorProviderInput,
  type IdeCursorRequest,
} from "./cursor-contract.ts";
import {
  IdeDocumentGeneration as IdeMonacoDocumentGeneration,
  IdeDocumentSequence,
  IdeMonacoModelVersion,
  makeIdeDocumentRef,
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
  IdeRootRefSchema,
  IdeSessionRefSchema,
  IdeTimestampSchema,
  IdeWorktreeRefSchema,
} from "./project-contract.ts";

export const ideCursorFixtureDigest = (character: string): `sha256:${string}` =>
  `sha256:${character.repeat(64).slice(0, 64)}`;

export const ideCursorFixtureDocumentText = "export const answer = 41\n";

const documentDigest = (): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(ideCursorFixtureDocumentText).digest("hex")}`;

export const ideCursorFixtureCapabilities = () =>
  IdeCursorCapabilitiesSchema.make({
    providerRef: "provider.fixture",
    modelRefs: ["model.fixture"],
    intents: ["complete", "next_edit", "ask", "change"],
    noFilesystemAccess: true,
    noShellAccess: true,
    identityBeforeCandidate: true,
    supportsCancellation: true,
    supportsOffline: true,
  });

export const ideCursorFixtureIdentity = (
  overrides: Readonly<{ provider?: string; model?: string }> = {},
): IdeCursorIdentityProgress => {
  const observedAt = IdeTimestampSchema.make("2026-07-19T12:00:00.000Z");
  const observed = (value: string) => ({
    value,
    evidence: { _tag: "Observed" as const, evidenceRef: `fixture.${value}`, observedAt },
  });
  const identity = IdeCursorExecutionIdentitySchema.make({
    harness: observed("harness.fixture"),
    provider: observed(overrides.provider ?? "provider.fixture"),
    model: observed(overrides.model ?? "model.fixture"),
    account: observed("account.fixture"),
    placementRef: IdePlacementRefSchema.make("ide.placement.fixture"),
    placementGeneration: IdePlacementGenerationSchema.make(1),
    indexPosture: "disabled",
    networkPosture: "offline",
  });
  return IdeCursorIdentityProgressSchema.make({
    requested: identity,
    admitted: identity,
    effective: identity,
    substitution: { _tag: "None" },
  });
};

export const ideCursorFixtureAnchor = (overrides: Partial<IdeCursorAnchor> = {}): IdeCursorAnchor =>
  IdeCursorAnchorSchema.make({
    projectRef: IdeProjectRefSchema.make("ide.project.fixture"),
    rootRef: IdeRootRefSchema.make("ide.root.fixture"),
    worktreeRef: IdeWorktreeRefSchema.make("ide.worktree.fixture"),
    attachmentRef: IdeAttachmentRefSchema.make("ide.attachment.fixture"),
    attachmentGeneration: IdeAttachmentGenerationSchema.make(1),
    sessionRef: IdeSessionRefSchema.make("ide.session.fixture"),
    sourceDocumentRef: makeIdeDocumentRef("ide.attachment.fixture.1", 0),
    sourceDocumentGeneration: IdeMonacoDocumentGeneration.make(0),
    fileRef: IdeFileRefSchema.make("ide.file.fixture"),
    documentRef: IdeDocumentRefSchema.make("ide.document.fixture"),
    documentGeneration: IdeDocumentGenerationSchema.make(1),
    documentSequence: IdeDocumentSequence.make(0),
    modelVersion: IdeMonacoModelVersion.make(1),
    selectionVersion: IdeCursorSelectionVersionSchema.make(0),
    pathRef: "src/app.ts",
    selection: { start: { line: 1, column: 1 }, end: { line: 1, column: 1 } },
    contentDigest: documentDigest(),
    ...overrides,
  });

export const ideCursorFixtureRequest = (
  suffix = "one",
  sequence = 1,
  overrides: Partial<IdeCursorRequest> = {},
): IdeCursorRequest =>
  IdeCursorRequestSchema.make({
    schemaVersion: "openagents.ide-cursor.v1",
    requestRef: IdeCursorRequestRefSchema.make(`ide.cursor-request.fixture.${suffix}`),
    attemptRef: IdeCursorAttemptRefSchema.make(`ide.cursor-attempt.fixture.${suffix}`),
    sequence: IdeCursorSequenceSchema.make(sequence),
    requestedAt: IdeTimestampSchema.make("2026-07-19T12:00:01.000Z"),
    anchor: ideCursorFixtureAnchor(),
    intent: { _tag: "Complete", acceptance: "all" },
    identity: ideCursorFixtureIdentity(),
    contextRef: IdeCursorContextRefSchema.make(`ide.cursor-context.fixture.${suffix}`),
    contextDigest: ideCursorFixtureDigest("b"),
    budget: { maxLatencyMs: 2_000, maxInputTokens: 4_096, maxOutputTokens: 1_024 },
    ...overrides,
  });

export const ideCursorFixtureInput = (
  request = ideCursorFixtureRequest(),
): IdeCursorProviderInput => {
  const manifest = ideAgentFixtureManifest();
  const document = ideAgentFixtureDocument();
  return IdeCursorProviderInputSchema.make({
    request,
    proposalContext: {
      attachment: manifest.attachment,
      manifestRef: manifest.manifestRef,
      turnRef: manifest.turnRef,
      conversationThreadRef: manifest.conversationThreadRef,
      bases: [{ fileRef: document.fileRef, pathRef: document.pathRef, base: ideAgentFixtureBase(document) }],
    },
    documentText: ideCursorFixtureDocumentText,
    context: [],
  });
};

export const ideCursorFixtureDisclosure = () =>
  IdeCursorDisclosureSchema.make({
    dataDestinations: [],
    usage: {
      input: { _tag: "Measured", value: 8, unit: "tokens" },
      output: { _tag: "Measured", value: 4, unit: "tokens" },
      cost: { _tag: "Measured", value: 0, unit: "usd_micros" },
    },
    noRemoteIndexDependency: true,
    secretsSent: false,
  });

export const ideCursorFixtureCandidate = (
  request = ideCursorFixtureRequest(),
  overrides: Partial<Extract<IdeCursorCandidate, { readonly _tag: "Completion" }>> = {},
): Extract<IdeCursorCandidate, { readonly _tag: "Completion" }> =>
  IdeCursorCandidateSchema.cases.Completion.make({
    schemaVersion: "openagents.ide-cursor.v1",
    candidateRef: IdeCursorCandidateRefSchema.make(
      `ide.cursor-candidate.fixture.${String(request.sequence)}`,
    ),
    requestRef: request.requestRef,
    attemptRef: request.attemptRef,
    sequence: request.sequence,
    anchor: request.anchor,
    identity: request.identity,
    disclosure: ideCursorFixtureDisclosure(),
    provenance: [{ sourceRef: "fixture.document", source: "document", freshness: "current" }],
    quality: { confidence: 0.9, syntaxChecked: true, diagnosticsChecked: true },
    staleness: { _tag: "Fresh" },
    createdAt: IdeTimestampSchema.make("2026-07-19T12:00:02.000Z"),
    resultDigest: ideCursorFixtureDigest("c"),
    replace: request.anchor.selection,
    text: "export const answer = 42\n",
    ...overrides,
  });

export const ideCursorFixtureDecision = (
  candidate: IdeCursorCandidate = ideCursorFixtureCandidate(),
  kind: "accept" | "undo" = "accept",
): IdeCursorDecision =>
  kind === "accept"
    ? IdeCursorDecisionSchema.make({
        _tag: "Accept",
        decisionRef: IdeCursorDecisionRefSchema.make("ide.cursor-decision.fixture.accept"),
        candidateRef: candidate.candidateRef,
        requestRef: candidate.requestRef,
        sequence: candidate.sequence,
        acceptedAt: IdeTimestampSchema.make("2026-07-19T12:00:03.000Z"),
        granularity: "all",
        resultDigest: candidate.resultDigest,
      })
    : IdeCursorDecisionSchema.make({
        _tag: "Undo",
        decisionRef: IdeCursorDecisionRefSchema.make("ide.cursor-decision.fixture.undo"),
        candidateRef: candidate.candidateRef,
        requestRef: candidate.requestRef,
        sequence: candidate.sequence,
        decidedAt: IdeTimestampSchema.make("2026-07-19T12:00:04.000Z"),
        resultDigest: candidate.anchor.contentDigest,
      });
