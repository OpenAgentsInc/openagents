import { createHash } from "node:crypto"
import { mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { afterEach, describe, expect, test } from "vite-plus/test"

import { openWorkspaceService, type DesktopWorkspaceService } from "../workspace-service.ts"
import {
  IdeAgentAttachmentSchema,
  IdeAgentContextManifestSchema,
  IdeAgentDecisionRefSchema,
  IdeAgentDecisionSchema,
  IdeAgentManifestRefSchema,
  IdeAgentOperationRefSchema,
  IdeAgentProposalBaseSchema,
  IdeAgentProposalSchema,
  IdeAgentReviewRefSchema,
} from "./agent-code-contract.ts"
import {
  ideAgentFixtureAttachment,
  ideAgentFixtureManifest,
  ideAgentFixtureProposal,
} from "./agent-code-fixture.ts"
import { openIdeAgentCodeHost } from "./agent-code-host.ts"
import {
  IdeDiskRevisionRefSchema,
  IdeDocumentGenerationSchema,
  IdeDocumentRefSchema,
  IdeFileRefSchema,
  IdeProposalRefSchema,
  IdeTimestampSchema,
} from "./project-contract.ts"

const roots: string[] = []
const workspaces: DesktopWorkspaceService[] = []
const makeRoot = (): string => {
  const root = mkdtempSync(path.join(tmpdir(), "openagents-ide-agent-host-"))
  roots.push(root)
  return root
}

afterEach(() => {
  while (workspaces.length > 0) workspaces.pop()!.dispose()
  while (roots.length > 0) rmSync(roots.pop()!, { recursive: true, force: true })
})

const suffix = (value: string): string => createHash("sha256").update(value).digest("hex").slice(0, 32)
const digest = (value: string): `sha256:${string}` => `sha256:${createHash("sha256").update(value).digest("hex")}`

const setup = async () => {
  const root = makeRoot()
  writeFileSync(path.join(root, "app.ts"), "export const answer = 41\n")
  const workspace = openWorkspaceService(root, { grantRef: "workspace.grant.agent-host" })
  workspaces.push(workspace)
  const persistencePath = path.join(root, ".agent-state", "agent-code.json")
  const host = await openIdeAgentCodeHost(workspace, { persistencePath })
  const attachment = IdeAgentAttachmentSchema.make({
    ...ideAgentFixtureAttachment(),
    grantRef: workspace.grantRef,
  })
  const manifest = IdeAgentContextManifestSchema.make({
    ...ideAgentFixtureManifest(),
    attachment,
  })
  return { root, workspace, persistencePath, host, attachment, manifest }
}

const admit = async (fixture: Awaited<ReturnType<typeof setup>>) => {
  expect((await fixture.host.command({ _tag: "Attach", attachment: fixture.attachment }))._tag).toBe("Succeeded")
  expect((await fixture.host.command({
    _tag: "AssembleManifest",
    input: { manifest: fixture.manifest, expectedAttachmentGeneration: fixture.attachment.attachmentGeneration },
  }))._tag).toBe("Succeeded")
}

const proposalForWorkspace = (
  fixture: Awaited<ReturnType<typeof setup>>,
  targetContent = "export const answer = 42\n",
) => {
  const opened = fixture.workspace.openDocument({ grantRef: fixture.workspace.grantRef, pathRef: "app.ts" })
  if (opened.state !== "available") throw new Error("fixture document unavailable")
  const document = opened.document
  const pathSuffix = suffix(document.pathRef)
  const documentRef = IdeDocumentRefSchema.make(`ide.document.workspace.${pathSuffix}`)
  const base = IdeAgentProposalBaseSchema.make({
    existed: true,
    content: document.content,
    diskRevisionRef: IdeDiskRevisionRefSchema.make(`ide.disk-revision.workspace.${suffix(document.revisionRef)}`),
    documentRef,
    documentGeneration: IdeDocumentGenerationSchema.make(1),
    gitSnapshotRef: null,
    gitSnapshotGeneration: null,
    checkpointRef: null,
    contentDigest: digest(document.content),
    encoding: document.encoding,
    lineEnding: document.lineEnding,
    mode: "regular",
  })
  return IdeAgentProposalSchema.make({
    ...ideAgentFixtureProposal(),
    proposalRef: IdeProposalRefSchema.make("ide.proposal.workspace-host"),
    attachment: fixture.attachment,
    manifestRef: IdeAgentManifestRefSchema.make(fixture.manifest.manifestRef),
    operations: [{
      _tag: "Edit",
      operationRef: IdeAgentOperationRefSchema.make("ide.agent-operation.workspace-host.edit"),
      fileRef: IdeFileRefSchema.make(`ide.file.workspace.${pathSuffix}`),
      pathRef: document.pathRef,
      base,
      policy: { encoding: "preserve", lineEnding: "preserve", mode: "preserve", symlink: "refuse" },
      documentRef,
      targetContent,
      targetContentDigest: digest(targetContent),
    }],
  })
}

describe("IDE-08 main-owned agent-code host", () => {
  test("applies and undoes through the canonical workspace authority", async () => {
    const fixture = await setup()
    await admit(fixture)
    const proposal = proposalForWorkspace(fixture)
    expect((await fixture.host.command({
      _tag: "SubmitProposal",
      input: { proposal, expectedAttachmentGeneration: fixture.attachment.attachmentGeneration },
    }))._tag).toBe("Succeeded")
    expect((await fixture.host.command({
      _tag: "BeginReview",
      input: {
        proposalRef: proposal.proposalRef,
        reviewRef: IdeAgentReviewRefSchema.make("ide.agent-review.workspace-host"),
        expectedAttachmentGeneration: fixture.attachment.attachmentGeneration,
      },
    }))._tag).toBe("Succeeded")
    const decision = IdeAgentDecisionSchema.make({
      decisionRef: IdeAgentDecisionRefSchema.make("ide.agent-decision.workspace-host"),
      proposalRef: proposal.proposalRef,
      decidedAt: IdeTimestampSchema.make("2026-07-19T15:00:00.000Z"),
      disposition: "accept",
      operationRefs: proposal.operations.map(operation => operation.operationRef),
      reason: null,
    })
    const accepted = await fixture.host.command({
      _tag: "Decide",
      decision,
      expectedAttachmentGeneration: fixture.attachment.attachmentGeneration,
    })
    expect(accepted._tag).toBe("Succeeded")
    if (accepted._tag !== "Succeeded") return
    const applied = await fixture.host.command({
      _tag: "Apply",
      input: {
        proposalRef: proposal.proposalRef,
        operationRefs: proposal.operations.map(operation => operation.operationRef),
        expectedAttachmentGeneration: fixture.attachment.attachmentGeneration,
        expectedProposalRevision: accepted.snapshot.revision,
      },
    })
    expect(applied._tag).toBe("Succeeded")
    expect(readFileSync(path.join(fixture.root, "app.ts"), "utf8")).toBe("export const answer = 42\n")
    if (applied._tag !== "Succeeded") return
    expect(applied.snapshot.evidence.map(fact => fact.kind)).toEqual([
      "diagnostics", "format", "test", "git_status", "git_diff", "delivery", "verification", "acceptance",
    ])
    expect(applied.snapshot.evidence.find(fact => fact.kind === "test")?.state._tag).toBe("Unavailable")
    expect(applied.snapshot.evidence.find(fact => fact.kind === "delivery")?.state._tag).toBe("Unavailable")
    const lifecycle = applied.snapshot.proposals.at(-1)?.lifecycle
    expect(lifecycle?._tag).toBe("Applied")
    if (lifecycle?._tag !== "Applied") return
    await fixture.host.dispose()
    const resumed = await openIdeAgentCodeHost(fixture.workspace, { persistencePath: fixture.persistencePath })
    expect((await resumed.snapshot()).proposals.at(-1)?.lifecycle._tag).toBe("Applied")
    const undone = await resumed.command({
      _tag: "Undo",
      input: {
        proposalRef: proposal.proposalRef,
        applyRef: lifecycle.applyRef,
        checkpointRef: lifecycle.checkpointRef,
        expectedAttachmentGeneration: fixture.attachment.attachmentGeneration,
      },
    })
    expect(undone._tag).toBe("Succeeded")
    expect(readFileSync(path.join(fixture.root, "app.ts"), "utf8")).toBe("export const answer = 41\n")
    await resumed.dispose()
  })

  test("recovers exact state and fences corrupt persistence until explicit reattachment", async () => {
    const fixture = await setup()
    await admit(fixture)
    await fixture.host.dispose()
    const resumed = await openIdeAgentCodeHost(fixture.workspace, { persistencePath: fixture.persistencePath })
    expect((await resumed.snapshot()).manifests).toHaveLength(1)
    await resumed.dispose()

    writeFileSync(fixture.persistencePath, "{not-json")
    const corrupt = await openIdeAgentCodeHost(fixture.workspace, { persistencePath: fixture.persistencePath })
    const refused = await corrupt.command({ _tag: "Stop", reason: "test" })
    expect(refused).toMatchObject({ _tag: "Refused", reason: "corrupt_persistence" })
    expect((await corrupt.command({ _tag: "Attach", attachment: fixture.attachment }))._tag).toBe("Succeeded")
    await corrupt.dispose()
  })

  test("refuses wrong grants and secret paths while the workspace withholds symlink targets", async () => {
    const fixture = await setup()
    const wrong = IdeAgentAttachmentSchema.make({ ...fixture.attachment, grantRef: "workspace.grant.other" })
    expect(await fixture.host.command({ _tag: "Attach", attachment: wrong })).toMatchObject({ _tag: "Refused", reason: "wrong_attachment" })
    await admit(fixture)

    writeFileSync(path.join(fixture.root, ".env"), "TOKEN=do-not-touch\n")
    symlinkSync(path.join(fixture.root, "app.ts"), path.join(fixture.root, "linked.ts"))
    const secretProposal = proposalForWorkspace(fixture)
    const secret = IdeAgentProposalSchema.make({
      ...secretProposal,
      proposalRef: IdeProposalRefSchema.make("ide.proposal.workspace-host.secret"),
      operations: secretProposal.operations.map(operation => ({
        ...operation,
        operationRef: IdeAgentOperationRefSchema.make("ide.agent-operation.workspace-host.secret"),
        pathRef: ".env",
        targetContent: "TOKEN=changed\n",
        targetContentDigest: digest("TOKEN=changed\n"),
      })),
    })
    expect((await fixture.host.command({
      _tag: "SubmitProposal",
      input: { proposal: secret, expectedAttachmentGeneration: fixture.attachment.attachmentGeneration },
    }))._tag).toBe("Succeeded")
    const review = await fixture.host.command({
      _tag: "BeginReview",
      input: { proposalRef: secret.proposalRef, reviewRef: IdeAgentReviewRefSchema.make("ide.agent-review.workspace-host.secret"), expectedAttachmentGeneration: fixture.attachment.attachmentGeneration },
    })
    expect(review._tag).toBe("Succeeded")
    const secretDecision = IdeAgentDecisionSchema.make({
      decisionRef: IdeAgentDecisionRefSchema.make("ide.agent-decision.workspace-host.secret"),
      proposalRef: secret.proposalRef,
      decidedAt: IdeTimestampSchema.make("2026-07-19T15:01:00.000Z"),
      disposition: "accept",
      operationRefs: secret.operations.map(operation => operation.operationRef),
      reason: null,
    })
    const accepted = await fixture.host.command({
      _tag: "Decide",
      decision: secretDecision,
      expectedAttachmentGeneration: fixture.attachment.attachmentGeneration,
    })
    expect(accepted._tag).toBe("Succeeded")
    if (accepted._tag === "Succeeded") {
      expect(await fixture.host.command({
        _tag: "Apply",
        input: {
          proposalRef: secret.proposalRef,
          operationRefs: secret.operations.map(operation => operation.operationRef),
          expectedAttachmentGeneration: fixture.attachment.attachmentGeneration,
          expectedProposalRevision: accepted.snapshot.revision,
        },
      })).toMatchObject({ _tag: "Refused", reason: "base_changed" })
    }
    expect(readFileSync(path.join(fixture.root, ".env"), "utf8")).toBe("TOKEN=do-not-touch\n")
    expect(fixture.workspace.openDocument({ grantRef: fixture.workspace.grantRef, pathRef: "linked.ts" }).state).toBe("unavailable")
  })
})
