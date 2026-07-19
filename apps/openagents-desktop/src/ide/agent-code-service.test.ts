import { Effect, Exit, Layer, Schema } from "effect"
import { describe, expect, test } from "vite-plus/test"

import {
  IdeAgentAttachmentRefSchema,
  IdeAgentAttachmentSchema,
  IdeAgentDecisionRefSchema,
  IdeAgentDecisionSchema,
  IdeAgentContextManifestSchema,
  IdeAgentEvidenceFactSchema,
  IdeAgentEvidenceStateSchema,
  IdeAgentManifestRefSchema,
  IdeAgentOperationRefSchema,
  IdeAgentPacketRefSchema,
  IdeAgentProposalBaseSchema,
  IdeAgentProposalSchema,
  IdeAgentProductSpecLineageSchema,
  IdeAgentReviewRefSchema,
  IdeAgentSpecRevisionRefSchema,
  IdeAgentTurnRefSchema,
  type IdeAgentProposalOperation,
} from "./agent-code-contract.ts"
import {
  IdeAgentCodeBaseChanged,
  IdeAgentCodeInvalidInput,
  IdeAgentCodeInvariantViolation,
  IdeAgentCodeService,
  IdeAgentCodeServiceErrorSchema,
  makeIdeAgentCodeLayer,
  makeIdeAgentCodeTestLayer,
  makeIdeAgentMemoryAuthorityLayer,
  type IdeAgentMemoryDocument,
} from "./agent-code-service.ts"
import {
  ideAgentFixtureAttachment,
  ideAgentFixtureBase,
  ideAgentFixtureContentDigest,
  ideAgentFixtureDecision,
  ideAgentFixtureDigest,
  ideAgentFixtureDocument,
  ideAgentFixtureManifest,
  ideAgentFixtureProposal,
} from "./agent-code-fixture.ts"
import {
  IdeAttachmentGenerationSchema,
  IdeDiskRevisionRefSchema,
  IdeDocumentGenerationSchema,
  IdeDocumentRefSchema,
  IdeEvidenceRefSchema,
  IdeFileRefSchema,
  IdeProposalRefSchema,
  IdeSessionRefSchema,
  IdeTimestampSchema,
  IdeWorktreeRefSchema,
} from "./project-contract.ts"

const run = <A, E>(
  effect: Effect.Effect<A, E, IdeAgentCodeService>,
  documents = [ideAgentFixtureDocument()],
  recovered?: Parameters<typeof makeIdeAgentCodeTestLayer>[1],
) => Effect.runPromise(effect.pipe(Effect.provide(makeIdeAgentCodeTestLayer(documents, recovered))))

const fixtureGeneration = ideAgentFixtureAttachment().attachmentGeneration

const prepareAccepted = Effect.gen(function* () {
  const service = yield* IdeAgentCodeService
  const attachment = ideAgentFixtureAttachment()
  const manifest = ideAgentFixtureManifest()
  const proposal = ideAgentFixtureProposal()
  yield* service.attach(attachment)
  yield* service.assembleManifest({ manifest, expectedAttachmentGeneration: attachment.attachmentGeneration })
  yield* service.submitProposal({ proposal, expectedAttachmentGeneration: attachment.attachmentGeneration })
  yield* service.beginReview({
    proposalRef: proposal.proposalRef,
    reviewRef: IdeAgentReviewRefSchema.make("ide.agent-review.fixture"),
    expectedAttachmentGeneration: attachment.attachmentGeneration,
  })
  return yield* service.decide(ideAgentFixtureDecision(proposal), attachment.attachmentGeneration)
})

describe("IdeAgentCodeService", () => {
  test("rejects a manifest whose disclosed included bytes exceed its exact budget", async () => {
    const failure = await run(Effect.gen(function* () {
      const service = yield* IdeAgentCodeService
      const attachment = ideAgentFixtureAttachment()
      const manifest = ideAgentFixtureManifest()
      yield* service.attach(attachment)
      return yield* service.assembleManifest({
        manifest: IdeAgentContextManifestSchema.make({ ...manifest, byteBudget: manifest.includedBytes - 1 }),
        expectedAttachmentGeneration: attachment.attachmentGeneration,
      }).pipe(Effect.flip)
    }))
    expect(failure).toBeInstanceOf(IdeAgentCodeInvariantViolation)
    if (failure instanceof IdeAgentCodeInvariantViolation) expect(failure.detail).toContain("exceeds")
  })

  test("rejects untrusted proposal content whose claimed digest does not match its bytes", async () => {
    const failure = await run(Effect.gen(function* () {
      const service = yield* IdeAgentCodeService
      const attachment = ideAgentFixtureAttachment()
      const original = ideAgentFixtureProposal()
      const proposal = IdeAgentProposalSchema.make({
        ...original,
        proposalRef: IdeProposalRefSchema.make("ide.proposal.fixture.bad-digest"),
        operations: original.operations.map(operation => operation._tag === "Edit"
          ? { ...operation, targetContentDigest: ideAgentFixtureDigest("0") }
          : operation),
      })
      yield* service.attach(attachment)
      yield* service.assembleManifest({ manifest: ideAgentFixtureManifest(), expectedAttachmentGeneration: fixtureGeneration })
      return yield* service.submitProposal({ proposal, expectedAttachmentGeneration: fixtureGeneration }).pipe(Effect.flip)
    }))
    expect(failure).toBeInstanceOf(IdeAgentCodeInvariantViolation)
    if (failure instanceof IdeAgentCodeInvariantViolation) expect(failure.detail).toContain("content digest")
  })

  test("runs attach → disclose → proposal → review → accept → canonical apply → evidence → undo", async () => {
    const result = await run(Effect.gen(function* () {
      const accepted = yield* prepareAccepted
      const service = yield* IdeAgentCodeService
      const proposal = ideAgentFixtureProposal()
      const applied = yield* service.apply({
        proposalRef: proposal.proposalRef,
        operationRefs: proposal.operations.map(operation => operation.operationRef),
        expectedAttachmentGeneration: fixtureGeneration,
        expectedProposalRevision: accepted.revision,
      })
      const appliedProposal = applied.proposals.find(candidate => candidate.proposalRef === proposal.proposalRef)!
      expect(appliedProposal.lifecycle._tag).toBe("Applied")
      expect(applied.checkpoints).toHaveLength(1)
      expect(applied.applyReceipts).toHaveLength(1)
      expect(applied.backlinks).toHaveLength(1)
      if (appliedProposal.lifecycle._tag !== "Applied") throw new Error("fixture did not apply")
      const fact = IdeAgentEvidenceFactSchema.make({
        evidenceRef: IdeEvidenceRefSchema.make("ide.evidence.fixture.diagnostics"),
        proposalRef: proposal.proposalRef,
        applyRef: appliedProposal.lifecycle.applyRef,
        postImageGeneration: 2,
        kind: "diagnostics",
        state: IdeAgentEvidenceStateSchema.cases.Passed.make({
          observedAt: IdeTimestampSchema.make("2026-07-19T12:00:05.000Z"),
          summary: "No diagnostics observed by the language service.",
        }),
        observedBy: "language_service",
        artifactRef: null,
        commitRef: null,
        lineage: null,
      })
      const evidenced = yield* service.recordEvidence(fact, fixtureGeneration)
      expect(evidenced.evidence[0]).toEqual(fact)
      const undone = yield* service.undo({
        proposalRef: proposal.proposalRef,
        applyRef: appliedProposal.lifecycle.applyRef,
        checkpointRef: appliedProposal.lifecycle.checkpointRef,
        expectedAttachmentGeneration: fixtureGeneration,
      })
      expect(undone.undoReceipts).toHaveLength(1)
      expect(undone.checkpoints[0]?.consumedByUndoRef).not.toBeNull()
      expect(undone.backlinks[0]?.resolution._tag).toBe("Historical")
      expect(undone.proposals[0]?.lifecycle._tag).toBe("Undone")
      expect(undone.evidence[0]?.state._tag).toBe("Stale")
      const receipt = yield* service.receipt()
      return { applied, undone, receipt }
    }))
    expect(result.receipt).toMatchObject({
      manifestCount: 1,
      includedItemCount: 1,
      omittedItemCount: 1,
      proposalCounts: { applied: 0, undone: 1 },
      evidenceCounts: { observed: 1, passed: 0, failed: 0 },
      containsPrivateContent: false,
    })
  })

  test("retains exact ProductSpec lineage across proposal, apply, backlink, and host evidence", async () => {
    const lineage = IdeAgentProductSpecLineageSchema.make({
      specRevisionRef: IdeAgentSpecRevisionRefSchema.make("ide.agent-spec-revision.fixture.8"),
      specDigest: ideAgentFixtureDigest("8"),
      criterionId: "Desktop AC-17",
      packetRef: IdeAgentPacketRefSchema.make("ide.agent-packet.fixture.8"),
      terminalOutcome: "pending",
      reviewPostImageRef: null,
    })
    const result = await run(Effect.gen(function* () {
      const service = yield* IdeAgentCodeService
      const attachment = ideAgentFixtureAttachment()
      const proposal = ideAgentFixtureProposal({
        proposalRef: IdeProposalRefSchema.make("ide.proposal.fixture.lineage"),
        lineage,
      })
      yield* service.attach(attachment)
      yield* service.assembleManifest({ manifest: ideAgentFixtureManifest(), expectedAttachmentGeneration: fixtureGeneration })
      yield* service.submitProposal({ proposal, expectedAttachmentGeneration: fixtureGeneration })
      yield* service.beginReview({
        proposalRef: proposal.proposalRef,
        reviewRef: IdeAgentReviewRefSchema.make("ide.agent-review.fixture.lineage"),
        expectedAttachmentGeneration: fixtureGeneration,
      })
      const accepted = yield* service.decide(IdeAgentDecisionSchema.make({
        ...ideAgentFixtureDecision(proposal),
        decisionRef: IdeAgentDecisionRefSchema.make("ide.agent-decision.fixture.lineage"),
      }), fixtureGeneration)
      const applied = yield* service.apply({
        proposalRef: proposal.proposalRef,
        operationRefs: proposal.operations.map(operation => operation.operationRef),
        expectedAttachmentGeneration: fixtureGeneration,
        expectedProposalRevision: accepted.revision,
      })
      const lifecycle = applied.proposals.find(candidate => candidate.proposalRef === proposal.proposalRef)?.lifecycle
      if (lifecycle?._tag !== "Applied") throw new Error("lineage fixture did not apply")
      const evidenced = yield* service.recordEvidence(IdeAgentEvidenceFactSchema.make({
        evidenceRef: IdeEvidenceRefSchema.make("ide.evidence.fixture.lineage"),
        proposalRef: proposal.proposalRef,
        applyRef: lifecycle.applyRef,
        postImageGeneration: 2,
        kind: "diagnostics",
        state: IdeAgentEvidenceStateSchema.cases.Passed.make({
          observedAt: IdeTimestampSchema.make("2026-07-19T12:00:05.000Z"),
          summary: "Host observed the lineage-bound post-image.",
        }),
        observedBy: "language_service",
        artifactRef: null,
        commitRef: null,
        lineage,
      }), fixtureGeneration)
      return { proposal: evidenced.proposals.at(-1), backlink: evidenced.backlinks.at(-1), evidence: evidenced.evidence.at(-1) }
    }))
    expect(result.proposal?.lineage).toEqual(lineage)
    expect(result.backlink).toMatchObject({ proposalRef: "ide.proposal.fixture.lineage", turnRef: "ide.agent-turn.fixture" })
    expect(result.evidence?.lineage).toEqual(lineage)
  })

  test("partial acceptance creates a new exact child proposal instead of screen-position splicing", async () => {
    const document = ideAgentFixtureDocument()
    const proposal = ideAgentFixtureProposal({
      proposalRef: IdeProposalRefSchema.make("ide.proposal.fixture.partial"),
      operations: [
        ideAgentFixtureProposal().operations[0]!,
        {
          _tag: "Create",
          operationRef: IdeAgentOperationRefSchema.make("ide.agent-operation.fixture.create"),
          fileRef: IdeFileRefSchema.make("ide.file.fixture.new"),
          pathRef: "src/new.ts",
          base: IdeAgentProposalBaseSchema.make({
            existed: false,
            content: null,
            diskRevisionRef: null,
            documentRef: null,
            documentGeneration: null,
            gitSnapshotRef: null,
            gitSnapshotGeneration: null,
            checkpointRef: null,
            contentDigest: null,
            encoding: "none",
            lineEnding: "none",
            mode: "none",
          }),
          policy: { encoding: "utf-8", lineEnding: "lf", mode: "regular", symlink: "refuse" },
          content: "export const created = true\n",
          contentDigest: ideAgentFixtureContentDigest("export const created = true\n"),
        },
      ],
    })
    const snapshot = await run(Effect.gen(function* () {
      const service = yield* IdeAgentCodeService
      const attachment = ideAgentFixtureAttachment()
      yield* service.attach(attachment)
      yield* service.assembleManifest({ manifest: ideAgentFixtureManifest(), expectedAttachmentGeneration: fixtureGeneration })
      yield* service.submitProposal({ proposal, expectedAttachmentGeneration: fixtureGeneration })
      const decision = IdeAgentDecisionSchema.make({
        decisionRef: IdeAgentDecisionRefSchema.make("ide.agent-decision.fixture.partial"),
        proposalRef: proposal.proposalRef,
        decidedAt: IdeTimestampSchema.make("2026-07-19T12:00:03.000Z"),
        disposition: "accept",
        operationRefs: [proposal.operations[0]!.operationRef],
        reason: null,
      })
      return yield* service.decide(decision, fixtureGeneration)
    }), [document])
    const parent = snapshot.proposals.find(candidate => candidate.proposalRef === proposal.proposalRef)!
    expect(parent.lifecycle).toMatchObject({
      _tag: "PartiallyAccepted",
      acceptedOperationRefs: [proposal.operations[0]!.operationRef],
      rejectedOperationRefs: [proposal.operations[1]!.operationRef],
    })
    expect(snapshot.proposals).toHaveLength(2)
    const child = snapshot.proposals.find(candidate => candidate.parentProposalRef === proposal.proposalRef)!
    expect(child.operations).toHaveLength(1)
    expect(child.lifecycle._tag).toBe("Accepted")
    const recovered = await run(Effect.gen(function* () {
      const service = yield* IdeAgentCodeService
      return yield* service.snapshot()
    }), [document], snapshot)
    expect(recovered.proposals.find(candidate => candidate.proposalRef === proposal.proposalRef)?.lifecycle._tag).toBe("PartiallyAccepted")
    expect(recovered.proposals.find(candidate => candidate.parentProposalRef === proposal.proposalRef)?.lifecycle._tag).toBe("Accepted")
  })

  test("applies and independently undoes a mixed create/edit/rename/delete transaction", async () => {
    const app = ideAgentFixtureDocument()
    const renamed = ideAgentFixtureDocument({
      pathRef: "src/old.ts",
      fileRef: IdeFileRefSchema.make("ide.file.fixture.old"),
      documentRef: IdeDocumentRefSchema.make("ide.document.fixture.old"),
      diskRevisionRef: IdeDiskRevisionRefSchema.make("ide.disk-revision.fixture.old.1"),
      content: "export const old = true\n",
      contentDigest: ideAgentFixtureDigest("e"),
    })
    const deleted = ideAgentFixtureDocument({
      pathRef: "src/delete.ts",
      fileRef: IdeFileRefSchema.make("ide.file.fixture.delete"),
      documentRef: IdeDocumentRefSchema.make("ide.document.fixture.delete"),
      diskRevisionRef: IdeDiskRevisionRefSchema.make("ide.disk-revision.fixture.delete.1"),
      content: "export const remove = true\n",
      contentDigest: ideAgentFixtureDigest("f"),
    })
    const createBase = IdeAgentProposalBaseSchema.make({
      existed: false, content: null, diskRevisionRef: null, documentRef: null, documentGeneration: null,
      gitSnapshotRef: null, gitSnapshotGeneration: null, checkpointRef: null, contentDigest: null,
      encoding: "none", lineEnding: "none", mode: "none",
    })
    const policy = { encoding: "preserve" as const, lineEnding: "preserve" as const, mode: "preserve" as const, symlink: "refuse" as const }
    const operations: ReadonlyArray<IdeAgentProposalOperation> = [
      {
        _tag: "Create",
        operationRef: IdeAgentOperationRefSchema.make("ide.agent-operation.mixed.create"),
        fileRef: IdeFileRefSchema.make("ide.file.mixed.create"),
        pathRef: "src/create.ts",
        base: createBase,
        policy: { ...policy, encoding: "utf-8", lineEnding: "lf", mode: "regular" },
        content: "export const created = true\n",
        contentDigest: ideAgentFixtureContentDigest("export const created = true\n"),
      },
      {
        _tag: "Edit",
        operationRef: IdeAgentOperationRefSchema.make("ide.agent-operation.mixed.edit"),
        fileRef: app.fileRef,
        pathRef: app.pathRef,
        base: ideAgentFixtureBase(app),
        policy,
        documentRef: app.documentRef,
        targetContent: "export const answer = 42\n",
        targetContentDigest: ideAgentFixtureContentDigest("export const answer = 42\n"),
      },
      {
        _tag: "Rename",
        operationRef: IdeAgentOperationRefSchema.make("ide.agent-operation.mixed.rename"),
        fileRef: renamed.fileRef,
        pathRef: renamed.pathRef,
        base: ideAgentFixtureBase(renamed),
        policy,
        documentRef: renamed.documentRef,
        targetPathRef: "src/renamed.ts",
      },
      {
        _tag: "Delete",
        operationRef: IdeAgentOperationRefSchema.make("ide.agent-operation.mixed.delete"),
        fileRef: deleted.fileRef,
        pathRef: deleted.pathRef,
        base: ideAgentFixtureBase(deleted),
        policy,
        documentRef: deleted.documentRef,
      },
    ]
    const attachment = ideAgentFixtureAttachment()
    const proposal = IdeAgentProposalSchema.make({
      schemaVersion: "openagents.desktop.ide-agent-code.v1",
      proposalRef: IdeProposalRefSchema.make("ide.proposal.mixed"),
      parentProposalRef: null,
      attachment,
      manifestRef: IdeAgentManifestRefSchema.make("ide.agent-manifest.fixture"),
      sessionRef: attachment.sessionRef,
      turnRef: IdeAgentTurnRefSchema.make("ide.agent-turn.fixture"),
      conversationThreadRef: "thread.fixture.agent-code",
      createdAt: IdeTimestampSchema.make("2026-07-19T12:00:02.000Z"),
      operations,
      lifecycle: { _tag: "Pending" },
      lineage: null,
    })
    const snapshot = await run(Effect.gen(function* () {
      const service = yield* IdeAgentCodeService
      yield* service.attach(attachment)
      yield* service.assembleManifest({ manifest: ideAgentFixtureManifest(), expectedAttachmentGeneration: fixtureGeneration })
      yield* service.submitProposal({ proposal, expectedAttachmentGeneration: fixtureGeneration })
      const accepted = yield* service.decide(IdeAgentDecisionSchema.make({
        decisionRef: IdeAgentDecisionRefSchema.make("ide.agent-decision.mixed.accept"),
        proposalRef: proposal.proposalRef,
        decidedAt: IdeTimestampSchema.make("2026-07-19T12:00:03.000Z"),
        disposition: "accept",
        operationRefs: operations.map(operation => operation.operationRef),
        reason: null,
      }), fixtureGeneration)
      const applied = yield* service.apply({
        proposalRef: proposal.proposalRef,
        operationRefs: operations.map(operation => operation.operationRef),
        expectedAttachmentGeneration: fixtureGeneration,
        expectedProposalRevision: accepted.revision,
      })
      const lifecycle = applied.proposals.find(candidate => candidate.proposalRef === proposal.proposalRef)!.lifecycle
      if (lifecycle._tag !== "Applied") throw new Error("mixed proposal did not apply")
      return yield* service.undo({
        proposalRef: proposal.proposalRef,
        applyRef: lifecycle.applyRef,
        checkpointRef: lifecycle.checkpointRef,
        expectedAttachmentGeneration: fixtureGeneration,
      })
    }), [app, renamed, deleted])
    expect(snapshot.applyReceipts[0]?.postImageRevisionRefs).toHaveLength(4)
    expect(snapshot.undoReceipts[0]?.restoredOperationRefs).toHaveLength(4)
    expect(snapshot.backlinks).toHaveLength(3)
    expect(snapshot.backlinks.every(backlink => backlink.resolution._tag === "Historical")).toBe(true)
  })

  test.each([
    ["dirty_document", { dirty: true }],
    ["symlink", { symlink: true }],
    ["binary", { contentClass: "binary" as const }],
    ["secret", { contentClass: "secret" as const }],
    ["private", { contentClass: "private" as const }],
    ["too_large", { contentClass: "too_large" as const }],
    ["revision_changed", { diskRevisionRef: IdeDiskRevisionRefSchema.make("ide.disk-revision.fixture.external.2") }],
  ] as const)("refuses %s bases without fuzzy apply", async (expectedReason, overrides) => {
    const result = await run(Effect.gen(function* () {
      const accepted = yield* prepareAccepted
      const service = yield* IdeAgentCodeService
      const proposal = ideAgentFixtureProposal()
      const outcome = yield* service.apply({
        proposalRef: proposal.proposalRef,
        operationRefs: proposal.operations.map(operation => operation.operationRef),
        expectedAttachmentGeneration: fixtureGeneration,
        expectedProposalRevision: accepted.revision,
      }).pipe(Effect.match({
        onFailure: failure => ({ _tag: "Failed" as const, failure }),
        onSuccess: snapshot => ({ _tag: "Succeeded" as const, snapshot }),
      }))
      return { outcome, snapshot: yield* service.snapshot() }
    }), [ideAgentFixtureDocument(overrides)])
    expect(result.outcome._tag).toBe("Failed")
    if (result.outcome._tag !== "Failed") return
    expect(result.outcome.failure).toBeInstanceOf(IdeAgentCodeBaseChanged)
    if (result.outcome.failure instanceof IdeAgentCodeBaseChanged) expect(result.outcome.failure.reason).toBe(expectedReason)
    expect(Exit.isSuccess(Schema.decodeUnknownExit(IdeAgentCodeServiceErrorSchema)(result.outcome.failure))).toBe(true)
    expect(result.snapshot.proposals[0]?.lifecycle).toMatchObject({
      _tag: "RebaseRequired",
      reason: expectedReason,
      conflictCount: 1,
    })
  })

  test("requires an explicit same-turn child proposal to rebase a changed base", async () => {
    const snapshot = await run(Effect.gen(function* () {
      const accepted = yield* prepareAccepted
      const service = yield* IdeAgentCodeService
      const original = ideAgentFixtureProposal()
      yield* service.apply({
        proposalRef: original.proposalRef,
        operationRefs: original.operations.map(operation => operation.operationRef),
        expectedAttachmentGeneration: fixtureGeneration,
        expectedProposalRevision: accepted.revision,
      }).pipe(Effect.ignore)
      const replacement = IdeAgentProposalSchema.make({
        ...original,
        proposalRef: IdeProposalRefSchema.make("ide.proposal.fixture.rebased"),
        parentProposalRef: original.proposalRef,
        createdAt: IdeTimestampSchema.make("2026-07-19T12:00:04.000Z"),
        lifecycle: { _tag: "Pending" },
      })
      return yield* service.rebase({
        proposalRef: original.proposalRef,
        replacementProposal: replacement,
        expectedAttachmentGeneration: fixtureGeneration,
      })
    }), [ideAgentFixtureDocument({ dirty: true })])
    expect(snapshot.proposals).toHaveLength(2)
    expect(snapshot.proposals[0]?.lifecycle).toMatchObject({
      _tag: "Superseded",
      replacementProposalRef: "ide.proposal.fixture.rebased",
    })
    expect(snapshot.proposals[1]).toMatchObject({ parentProposalRef: ideAgentFixtureProposal().proposalRef, lifecycle: { _tag: "Pending" } })
  })

  test("fences a late proposal after an attachment generation changes", async () => {
    const exit = await run(Effect.gen(function* () {
      const service = yield* IdeAgentCodeService
      const first = ideAgentFixtureAttachment()
      yield* service.attach(first)
      yield* service.assembleManifest({ manifest: ideAgentFixtureManifest(), expectedAttachmentGeneration: fixtureGeneration })
      yield* service.attach({
        ...first,
        agentAttachmentRef: first.agentAttachmentRef,
        attachmentGeneration: IdeAttachmentGenerationSchema.make(2),
      })
      return yield* service.submitProposal({
        proposal: ideAgentFixtureProposal(),
        expectedAttachmentGeneration: fixtureGeneration,
      }).pipe(Effect.flip)
    }))
    expect(exit._tag).toBe("IdeAgentCode.StaleGeneration")
  })

  test("keeps equal relative paths in parallel worktree attachments isolated", async () => {
    const applyLane = (lane: "left" | "right", targetContent: string) => {
      const attachment = IdeAgentAttachmentSchema.make({
        ...ideAgentFixtureAttachment(),
        agentAttachmentRef: IdeAgentAttachmentRefSchema.make(`ide.agent-attachment.fixture.${lane}`),
        worktreeRef: IdeWorktreeRefSchema.make(`ide.worktree.fixture.${lane}`),
        sessionRef: IdeSessionRefSchema.make(`ide.session.fixture.${lane}`),
      })
      const manifestRef = IdeAgentManifestRefSchema.make(`ide.agent-manifest.fixture.${lane}`)
      const manifest = IdeAgentContextManifestSchema.make({ ...ideAgentFixtureManifest(), attachment, manifestRef })
      const original = ideAgentFixtureProposal()
      const proposal = IdeAgentProposalSchema.make({
        ...original,
        proposalRef: IdeProposalRefSchema.make(`ide.proposal.fixture.${lane}`),
        attachment,
        manifestRef,
        sessionRef: attachment.sessionRef,
        operations: original.operations.map(operation => operation._tag === "Edit" ? {
          ...operation,
          operationRef: IdeAgentOperationRefSchema.make(`ide.agent-operation.fixture.${lane}`),
          targetContent,
          targetContentDigest: ideAgentFixtureContentDigest(targetContent),
        } : operation),
      })
      return run(Effect.gen(function* () {
        const service = yield* IdeAgentCodeService
        yield* service.attach(attachment)
        yield* service.assembleManifest({ manifest, expectedAttachmentGeneration: attachment.attachmentGeneration })
        yield* service.submitProposal({ proposal, expectedAttachmentGeneration: attachment.attachmentGeneration })
        const accepted = yield* service.decide(IdeAgentDecisionSchema.make({
          ...ideAgentFixtureDecision(proposal),
          decisionRef: IdeAgentDecisionRefSchema.make(`ide.agent-decision.fixture.${lane}`),
        }), attachment.attachmentGeneration)
        return yield* service.apply({
          proposalRef: proposal.proposalRef,
          operationRefs: proposal.operations.map(operation => operation.operationRef),
          expectedAttachmentGeneration: attachment.attachmentGeneration,
          expectedProposalRevision: accepted.revision,
        })
      }))
    }
    const [left, right] = await Promise.all([
      applyLane("left", "export const lane = 'left'\n"),
      applyLane("right", "export const lane = 'right'\n"),
    ])
    expect(left.attachment?.worktreeRef).toBe("ide.worktree.fixture.left")
    expect(right.attachment?.worktreeRef).toBe("ide.worktree.fixture.right")
    expect(left.applyReceipts[0]?.postImageRevisionRefs[0]?.contentDigest).not.toBe(
      right.applyReceipts[0]?.postImageRevisionRefs[0]?.contentDigest,
    )
  })

  test("reconciles exact runtime retries without duplicating manifests, proposals, reviews, or decisions", async () => {
    const result = await run(Effect.gen(function* () {
      const service = yield* IdeAgentCodeService
      const attachment = ideAgentFixtureAttachment()
      const manifest = ideAgentFixtureManifest()
      const proposal = ideAgentFixtureProposal()
      const decision = ideAgentFixtureDecision(proposal)
      yield* service.attach(attachment)
      const manifested = yield* service.assembleManifest({ manifest, expectedAttachmentGeneration: fixtureGeneration })
      const manifestedRetry = yield* service.assembleManifest({ manifest, expectedAttachmentGeneration: fixtureGeneration })
      const submitted = yield* service.submitProposal({ proposal, expectedAttachmentGeneration: fixtureGeneration })
      const submittedRetry = yield* service.submitProposal({ proposal, expectedAttachmentGeneration: fixtureGeneration })
      const reviewing = yield* service.beginReview({
        proposalRef: proposal.proposalRef,
        reviewRef: IdeAgentReviewRefSchema.make("ide.agent-review.fixture.retry"),
        expectedAttachmentGeneration: fixtureGeneration,
      })
      const reviewingRetry = yield* service.beginReview({
        proposalRef: proposal.proposalRef,
        reviewRef: IdeAgentReviewRefSchema.make("ide.agent-review.fixture.retry"),
        expectedAttachmentGeneration: fixtureGeneration,
      })
      const decided = yield* service.decide(decision, fixtureGeneration)
      const decidedRetry = yield* service.decide(decision, fixtureGeneration)
      return { manifested, manifestedRetry, submitted, submittedRetry, reviewing, reviewingRetry, decided, decidedRetry }
    }))
    expect(result.manifestedRetry.revision).toBe(result.manifested.revision)
    expect(result.submittedRetry.revision).toBe(result.submitted.revision)
    expect(result.reviewingRetry.revision).toBe(result.reviewing.revision)
    expect(result.decidedRetry.revision).toBe(result.decided.revision)
    expect(result.decidedRetry.manifests).toHaveLength(1)
    expect(result.decidedRetry.proposals).toHaveLength(1)
    expect(result.decidedRetry.decisions).toHaveLength(1)
  })

  test("recovers reviewed state from a valid snapshot and rejects corrupt persistence at acquisition", async () => {
    const recoveredSeed = await run(Effect.gen(function* () {
      const service = yield* IdeAgentCodeService
      const attachment = ideAgentFixtureAttachment()
      const proposal = ideAgentFixtureProposal()
      yield* service.attach(attachment)
      yield* service.assembleManifest({ manifest: ideAgentFixtureManifest(), expectedAttachmentGeneration: fixtureGeneration })
      yield* service.submitProposal({ proposal, expectedAttachmentGeneration: fixtureGeneration })
      return yield* service.beginReview({
        proposalRef: proposal.proposalRef,
        reviewRef: IdeAgentReviewRefSchema.make("ide.agent-review.recovered"),
        expectedAttachmentGeneration: fixtureGeneration,
      })
    }))
    const restored = await Effect.runPromise(Effect.gen(function* () {
      const service = yield* IdeAgentCodeService
      return yield* service.snapshot()
    }).pipe(Effect.provide(makeIdeAgentCodeTestLayer([ideAgentFixtureDocument()], recoveredSeed))))
    expect(restored.proposals[0]?.lifecycle._tag).toBe("Reviewing")
    const authorityLayer = makeIdeAgentMemoryAuthorityLayer([ideAgentFixtureDocument()])
    const invalidLayer = makeIdeAgentCodeLayer({ ...recoveredSeed, checkpoints: [{ rawPath: "/private" }] }).pipe(
      Layer.provide(authorityLayer),
    )
    const corrupt = await Effect.runPromise(Effect.gen(function* () {
      yield* IdeAgentCodeService
    }).pipe(Effect.provide(invalidLayer), Effect.exit))
    expect(Exit.isFailure(corrupt)).toBe(true)
    if (Exit.isFailure(corrupt)) expect(String(corrupt.cause)).toContain("IdeAgentCode.InvalidInput")
  })

  test("purges expired preimages and marks historical backlinks unavailable on recovery", async () => {
    const retained = await run(Effect.gen(function* () {
      const accepted = yield* prepareAccepted
      const service = yield* IdeAgentCodeService
      const proposal = ideAgentFixtureProposal()
      const applied = yield* service.apply({
        proposalRef: proposal.proposalRef,
        operationRefs: proposal.operations.map(operation => operation.operationRef),
        expectedAttachmentGeneration: fixtureGeneration,
        expectedProposalRevision: accepted.revision,
      })
      const lifecycle = applied.proposals[0]?.lifecycle
      if (lifecycle?._tag !== "Applied") throw new Error("retention fixture did not apply")
      return yield* service.undo({
        proposalRef: proposal.proposalRef,
        applyRef: lifecycle.applyRef,
        checkpointRef: lifecycle.checkpointRef,
        expectedAttachmentGeneration: fixtureGeneration,
      })
    }))
    expect(retained.checkpoints).toHaveLength(1)
    const recovered = await Effect.runPromise(Effect.gen(function* () {
      const service = yield* IdeAgentCodeService
      return yield* service.snapshot()
    }).pipe(Effect.provide(makeIdeAgentCodeTestLayer([ideAgentFixtureDocument()], retained, {
      now: () => IdeTimestampSchema.make("2099-01-01T00:00:00.000Z"),
    }))))
    expect(recovered.checkpoints).toHaveLength(0)
    expect(recovered.backlinks[0]?.resolution).toEqual({ _tag: "Unavailable", reason: "retention_expired" })
  })
})
