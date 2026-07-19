import { createHash } from "node:crypto"

import {
  IdeAgentAttachmentRefSchema,
  IdeAgentAttachmentSchema,
  IdeAgentContextItemRefSchema,
  IdeAgentContextItemSchema,
  IdeAgentContextManifestSchema,
  IdeAgentDecisionRefSchema,
  IdeAgentDecisionSchema,
  IdeAgentEffectiveRuntimeSchema,
  IdeAgentManifestRefSchema,
  IdeAgentOperationRefSchema,
  IdeAgentProposalBaseSchema,
  IdeAgentProposalSchema,
  IdeAgentTurnRefSchema,
  type IdeAgentContextManifest,
  type IdeAgentDecision,
  type IdeAgentProposal,
} from "./agent-code-contract.ts"
import { IdeAgentAuthorityFileSchema, type IdeAgentAuthorityFile } from "./agent-code-service.ts"
import {
  IdeAttachmentGenerationSchema,
  IdeDiskRevisionRefSchema,
  IdeDocumentGenerationSchema,
  IdeDocumentRefSchema,
  IdeFileRefSchema,
  IdePlacementGenerationSchema,
  IdePlacementRefSchema,
  IdeProjectRefSchema,
  IdeProposalRefSchema,
  IdeRootRefSchema,
  IdeSessionRefSchema,
  IdeTimestampSchema,
  IdeWorktreeRefSchema,
} from "./project-contract.ts"

export const ideAgentFixtureDigest = (character: string): `sha256:${string}` =>
  `sha256:${character.repeat(64).slice(0, 64)}`

export const ideAgentFixtureContentDigest = (content: string): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(content).digest("hex")}`

export const ideAgentFixtureAttachment = () => IdeAgentAttachmentSchema.make({
  schemaVersion: "openagents.desktop.ide-agent-code.v1",
  agentAttachmentRef: IdeAgentAttachmentRefSchema.make("ide.agent-attachment.fixture"),
  projectRef: IdeProjectRefSchema.make("ide.project.fixture"),
  rootRef: IdeRootRefSchema.make("ide.root.fixture"),
  worktreeRef: IdeWorktreeRefSchema.make("ide.worktree.fixture"),
  sessionRef: IdeSessionRefSchema.make("ide.session.fixture"),
  attachmentGeneration: IdeAttachmentGenerationSchema.make(1),
  placementGeneration: IdePlacementGenerationSchema.make(1),
  grantRef: "workspace.grant.fixture",
  attachedAt: IdeTimestampSchema.make("2026-07-19T12:00:00.000Z"),
  expiresAt: null,
})

export const ideAgentFixtureRuntime = () => IdeAgentEffectiveRuntimeSchema.make({
  harnessRef: "harness.fixture",
  modelRef: "model.fixture",
  providerRef: "provider.fixture",
  accountRef: "account.fixture",
  placementRef: IdePlacementRefSchema.make("ide.placement.fixture"),
  placementGeneration: IdePlacementGenerationSchema.make(1),
  toolPolicyRef: "tools.proposal-only",
  permissionMode: "proposal_only",
  sandboxRef: "sandbox.fixture",
  memoryPolicyRef: "memory.turn-only",
  instructionPolicyRef: "instructions.fixture",
  semanticRetrieval: "disabled",
})

export const ideAgentFixtureDocument = (
  overrides: Partial<IdeAgentAuthorityFile> = {},
): IdeAgentAuthorityFile => IdeAgentAuthorityFileSchema.make({
  pathRef: "src/app.ts",
  fileRef: IdeFileRefSchema.make("ide.file.fixture.app"),
  documentRef: IdeDocumentRefSchema.make("ide.document.fixture.app"),
  documentGeneration: IdeDocumentGenerationSchema.make(1),
  diskRevisionRef: IdeDiskRevisionRefSchema.make("ide.disk-revision.fixture.app.1"),
  content: "export const answer = 41\n",
  contentDigest: ideAgentFixtureDigest("a"),
  encoding: "utf-8",
  lineEnding: "lf",
  mode: "regular",
  dirty: false,
  symlink: false,
  contentClass: "text",
  ...overrides,
})

export const ideAgentFixtureManifest = (): IdeAgentContextManifest => {
  const attachment = ideAgentFixtureAttachment()
  const document = ideAgentFixtureDocument()
  const included = IdeAgentContextItemSchema.make({
    contextItemRef: IdeAgentContextItemRefSchema.make("ide.agent-context-item.fixture.file"),
    source: {
      _tag: "File",
      selectedBy: "user",
      sourceGeneration: 1,
      fileRef: document.fileRef,
      documentRef: document.documentRef,
      pathRef: document.pathRef,
      documentGeneration: document.documentGeneration,
      diskRevisionRef: document.diskRevisionRef,
    },
    disposition: { _tag: "Included", reason: "explicit_user_selection" },
    destination: { _tag: "HarnessPrompt", harnessRef: "harness.fixture" },
    freshness: "current",
    sensitivity: "workspace",
    retention: "turn_only",
    byteEstimate: 25,
    tokenEstimate: 7,
    truncated: false,
    label: "src/app.ts",
    excerpt: document.content,
  })
  const omitted = IdeAgentContextItemSchema.make({
    contextItemRef: IdeAgentContextItemRefSchema.make("ide.agent-context-item.fixture.semantic"),
    source: {
      _tag: "SemanticRetrieval",
      selectedBy: "retrieval",
      sourceGeneration: 1,
      resultRef: "retrieval.semantic.disabled",
      queryDigest: ideAgentFixtureDigest("b"),
    },
    disposition: { _tag: "Omitted", reason: "retrieval_disabled", detail: "Semantic retrieval is disabled; explicit and lexical context remains available." },
    destination: { _tag: "Withheld", reason: "retrieval disabled" },
    freshness: "unavailable",
    sensitivity: "workspace",
    retention: "withheld",
    byteEstimate: 0,
    tokenEstimate: 0,
    truncated: false,
    label: "Optional semantic retrieval",
    excerpt: null,
  })
  return IdeAgentContextManifestSchema.make({
    schemaVersion: "openagents.desktop.ide-agent-code.v1",
    manifestRef: IdeAgentManifestRefSchema.make("ide.agent-manifest.fixture"),
    attachment,
    turnRef: IdeAgentTurnRefSchema.make("ide.agent-turn.fixture"),
    conversationThreadRef: "thread.fixture.agent-code",
    createdAt: IdeTimestampSchema.make("2026-07-19T12:00:01.000Z"),
    effectiveRuntime: ideAgentFixtureRuntime(),
    items: [included, omitted],
    includedBytes: 25,
    includedTokens: 7,
    omittedCount: 1,
    byteBudget: 64_000,
    tokenBudget: 16_000,
    exportable: true,
    rebuildable: true,
    deletionPolicyRef: "retention.turn-only",
  })
}

export const ideAgentFixtureBase = (document = ideAgentFixtureDocument()) =>
  IdeAgentProposalBaseSchema.make({
    existed: true,
    content: document.content,
    diskRevisionRef: document.diskRevisionRef,
    documentRef: document.documentRef,
    documentGeneration: document.documentGeneration,
    gitSnapshotRef: null,
    gitSnapshotGeneration: null,
    checkpointRef: null,
    contentDigest: document.contentDigest,
    encoding: document.encoding,
    lineEnding: document.lineEnding,
    mode: document.mode,
  })

export const ideAgentFixtureProposal = (
  overrides: Partial<IdeAgentProposal> = {},
): IdeAgentProposal => {
  const attachment = ideAgentFixtureAttachment()
  const document = ideAgentFixtureDocument()
  return IdeAgentProposalSchema.make({
    schemaVersion: "openagents.desktop.ide-agent-code.v1",
    proposalRef: IdeProposalRefSchema.make("ide.proposal.fixture"),
    parentProposalRef: null,
    attachment,
    manifestRef: IdeAgentManifestRefSchema.make("ide.agent-manifest.fixture"),
    sessionRef: attachment.sessionRef,
    turnRef: IdeAgentTurnRefSchema.make("ide.agent-turn.fixture"),
    conversationThreadRef: "thread.fixture.agent-code",
    createdAt: IdeTimestampSchema.make("2026-07-19T12:00:02.000Z"),
    operations: [{
      _tag: "Edit",
      operationRef: IdeAgentOperationRefSchema.make("ide.agent-operation.fixture.edit"),
      fileRef: document.fileRef,
      pathRef: document.pathRef,
      base: ideAgentFixtureBase(document),
      policy: { encoding: "preserve", lineEnding: "preserve", mode: "preserve", symlink: "refuse" },
      documentRef: document.documentRef,
      targetContent: "export const answer = 42\n",
      targetContentDigest: ideAgentFixtureContentDigest("export const answer = 42\n"),
    }],
    lifecycle: { _tag: "Pending" },
    lineage: null,
    ...overrides,
  })
}

export const ideAgentFixtureDecision = (
  proposal = ideAgentFixtureProposal(),
  disposition: "accept" | "reject" = "accept",
): IdeAgentDecision => IdeAgentDecisionSchema.make({
  decisionRef: IdeAgentDecisionRefSchema.make(`ide.agent-decision.fixture.${disposition}`),
  proposalRef: proposal.proposalRef,
  decidedAt: IdeTimestampSchema.make("2026-07-19T12:00:03.000Z"),
  disposition,
  operationRefs: proposal.operations.map(operation => operation.operationRef),
  reason: disposition === "reject" ? "Fixture refusal" : null,
})
