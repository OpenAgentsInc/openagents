import { Exit, Schema } from "effect"
import { describe, expect, test } from "vite-plus/test"

import {
  IdeAgentCodeCommandSchema,
  IdeAgentCodeReceiptSchema,
  IdeAgentCodeSnapshotSchema,
  IdeAgentContextManifestSchema,
  IdeAgentProposalSchema,
  decodeIdeAgentCodeCommand,
  decodeIdeAgentCodeSnapshot,
  decodeIdeAgentContextManifest,
  decodeIdeAgentProposal,
  emptyIdeAgentCodeSnapshot,
} from "./agent-code-contract.ts"
import {
  ideAgentFixtureAttachment,
  ideAgentFixtureDecision,
  ideAgentFixtureManifest,
  ideAgentFixtureProposal,
} from "./agent-code-fixture.ts"

describe("IDE-08 agent-code contract", () => {
  test("derives every boundary type from one identified Effect Schema graph", () => {
    const manifest = ideAgentFixtureManifest()
    const proposal = ideAgentFixtureProposal()
    expect(Schema.decodeUnknownExit(IdeAgentContextManifestSchema)(manifest)._tag).toBe("Success")
    expect(Schema.decodeUnknownExit(IdeAgentProposalSchema)(proposal)._tag).toBe("Success")
    expect(Schema.decodeUnknownExit(IdeAgentCodeSnapshotSchema)(emptyIdeAgentCodeSnapshot())._tag).toBe("Success")
    expect(decodeIdeAgentContextManifest(manifest)).toEqual(manifest)
    expect(decodeIdeAgentProposal(proposal)).toEqual(proposal)
    expect(decodeIdeAgentCodeSnapshot(emptyIdeAgentCodeSnapshot())).not.toBeNull()
  })

  test("discloses explicit context and a typed semantic-retrieval omission when embeddings are off", () => {
    const manifest = ideAgentFixtureManifest()
    expect(manifest.effectiveRuntime.semanticRetrieval).toBe("disabled")
    expect(manifest.items).toHaveLength(2)
    expect(manifest.items[0]).toMatchObject({
      source: { _tag: "File", selectedBy: "user" },
      disposition: { _tag: "Included", reason: "explicit_user_selection" },
      freshness: "current",
      retention: "turn_only",
    })
    expect(manifest.items[1]).toMatchObject({
      source: { _tag: "SemanticRetrieval" },
      disposition: { _tag: "Omitted", reason: "retrieval_disabled" },
      destination: { _tag: "Withheld" },
    })
    expect(manifest.includedBytes).toBe(25)
    expect(manifest.includedTokens).toBe(7)
    expect(manifest.omittedCount).toBe(1)
  })

  test("admits the initial Monaco document generation without rewriting its identity", () => {
    const manifest = ideAgentFixtureManifest()
    const initialFile = {
      ...manifest.items[0],
      source: {
        ...manifest.items[0]!.source,
        sourceGeneration: 0,
        documentGeneration: 1,
      },
    }
    const initialManifest = {
      ...manifest,
      items: [initialFile, ...manifest.items.slice(1)],
    }

    expect(Schema.decodeUnknownExit(IdeAgentContextManifestSchema)(initialManifest)._tag).toBe("Success")
    expect(decodeIdeAgentContextManifest(initialManifest)?.items[0]?.source.sourceGeneration).toBe(0)
  })

  test("rejects malformed refs, absolute paths, raw proposal variants, and overlong content", () => {
    const proposal = ideAgentFixtureProposal()
    expect(decodeIdeAgentProposal({ ...proposal, proposalRef: "proposal-without-domain" })).toBeNull()
    expect(decodeIdeAgentProposal({
      ...proposal,
      operations: [{ ...proposal.operations[0], pathRef: "/Users/private/root.ts" }],
    })).toBeNull()
    expect(decodeIdeAgentProposal({ ...proposal, lifecycle: { kind: "Pending" } })).toBeNull()
    expect(decodeIdeAgentProposal({
      ...proposal,
      operations: [{ ...proposal.operations[0], targetContent: "x".repeat(1_000_001) }],
    })).toBeNull()
  })

  test("decodes command transport at entry and refuses provider-shaped parallel envelopes", () => {
    const command = IdeAgentCodeCommandSchema.cases.Attach.make({ attachment: ideAgentFixtureAttachment() })
    expect(decodeIdeAgentCodeCommand(command)).toEqual(command)
    expect(decodeIdeAgentCodeCommand({ type: "attach", root: "/tmp/private" })).toBeNull()
    expect(decodeIdeAgentCodeCommand({
      _tag: "Decide",
      decision: ideAgentFixtureDecision(),
      expectedAttachmentGeneration: "1",
    })).toBeNull()
  })

  test("public receipt schema makes private content structurally impossible", () => {
    const receipt = IdeAgentCodeReceiptSchema.make({
      schemaVersion: "openagents.desktop.ide-agent-code.v1",
      lifecycle: "attached",
      attachmentRef: ideAgentFixtureAttachment().agentAttachmentRef,
      projectRef: ideAgentFixtureAttachment().projectRef,
      worktreeRef: ideAgentFixtureAttachment().worktreeRef,
      attachmentGeneration: ideAgentFixtureAttachment().attachmentGeneration,
      manifestCount: 1,
      includedItemCount: 1,
      omittedItemCount: 1,
      proposalCounts: { pending: 1, reviewing: 0, partial: 0, applied: 0, undone: 0, refused: 0, stale: 0 },
      checkpointCount: 0,
      backlinkCount: 0,
      evidenceCounts: { observed: 0, passed: 0, failed: 0 },
      containsPrivateContent: false,
    })
    const encoded = JSON.stringify(receipt)
    expect(encoded).not.toContain("src/app.ts")
    expect(encoded).not.toContain("export const")
    expect(encoded).not.toContain("harness.fixture")
    expect(Exit.isSuccess(Schema.decodeUnknownExit(IdeAgentCodeReceiptSchema)(receipt))).toBe(true)
    expect(Exit.isFailure(Schema.decodeUnknownExit(IdeAgentCodeReceiptSchema)({ ...receipt, containsPrivateContent: true }))).toBe(true)
  })
})
