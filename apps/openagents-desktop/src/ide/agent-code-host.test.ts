import { createHash } from "node:crypto"
import { mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { afterEach, describe, expect, test } from "vite-plus/test"

import {
  openWorkspaceService,
  type DesktopWorkspaceService,
  type WorkspaceDocumentIo,
} from "../workspace-service.ts"
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
  decodeIdeAgentCodeSnapshot,
} from "./agent-code-contract.ts"
import {
  ideAgentFixtureAttachment,
  ideAgentFixtureManifest,
  ideAgentFixtureProposal,
} from "./agent-code-fixture.ts"
import { openIdeAgentCodeHost } from "./agent-code-host.ts"
import type {
  IdePortableMutationAuthority,
  IdePortableMutationPermit,
} from "./portable-mutation-authority.ts"
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

const setup = async (options: Readonly<{
  mutationAuthority?: IdePortableMutationAuthority
  documentIo?: WorkspaceDocumentIo
}> = {}) => {
  const root = makeRoot()
  writeFileSync(path.join(root, "app.ts"), "export const answer = 41\n")
  const workspace = openWorkspaceService(root, {
    grantRef: "workspace.grant.agent-host",
    mutationAuthority: options.mutationAuthority,
    documentIo: options.documentIo,
  })
  workspaces.push(workspace)
  const persistencePath = path.join(root, ".agent-state", "agent-code.json")
  const host = await openIdeAgentCodeHost(workspace, {
    persistencePath,
    mutationAuthority: options.mutationAuthority,
  })
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

const makePortableAuthority = (initialGeneration = 1): Readonly<{
  authority: IdePortableMutationAuthority
  generation: () => number
  setGeneration: (generation: number) => void
}> => {
  let generation = initialGeneration
  const authority: IdePortableMutationAuthority = {
    authorize: grantRef => ({
      _tag: "Permitted",
      permit: {
        _tag: "Portable",
        key: `portable:${grantRef}:ide.session.fixture:${generation}`,
        grantRef,
        sessionRef: "ide.session.fixture",
        workContextRef: "work-context.fixture",
        attachmentRef: "portable.attachment.fixture",
        generation,
        targetRef: "portable.target.local",
      },
    }),
    reauthorize: (permit: IdePortableMutationPermit) => permit._tag === "Portable" && permit.generation === generation,
  }
  return {
    authority,
    generation: () => generation,
    setGeneration: next => { generation = next },
  }
}

const admit = async (fixture: Awaited<ReturnType<typeof setup>>) => {
  expect((await fixture.host.command({ _tag: "Attach", attachment: fixture.attachment }))._tag).toBe("Succeeded")
  expect((await fixture.host.command({
    _tag: "AssembleManifest",
    input: { manifest: fixture.manifest, expectedAttachmentGeneration: fixture.attachment.attachmentGeneration },
  }))._tag).toBe("Succeeded")
}

const editOperationForWorkspace = (
  fixture: Awaited<ReturnType<typeof setup>>,
  pathRef: string,
  targetContent: string,
  refSuffix: string,
) => {
  const opened = fixture.workspace.openDocument({ grantRef: fixture.workspace.grantRef, pathRef })
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
  return {
    _tag: "Edit" as const,
    operationRef: IdeAgentOperationRefSchema.make(`ide.agent-operation.workspace-host.${refSuffix}`),
    fileRef: IdeFileRefSchema.make(`ide.file.workspace.${pathSuffix}`),
    pathRef: document.pathRef,
    base,
    policy: { encoding: "preserve" as const, lineEnding: "preserve" as const, mode: "preserve" as const, symlink: "refuse" as const },
    documentRef,
    targetContent,
    targetContentDigest: digest(targetContent),
  }
}

const proposalForWorkspace = (
  fixture: Awaited<ReturnType<typeof setup>>,
  targetContent = "export const answer = 42\n",
) => {
  const operation = editOperationForWorkspace(fixture, "app.ts", targetContent, "edit")
  return IdeAgentProposalSchema.make({
    ...ideAgentFixtureProposal(),
    proposalRef: IdeProposalRefSchema.make("ide.proposal.workspace-host"),
    attachment: fixture.attachment,
    manifestRef: IdeAgentManifestRefSchema.make(fixture.manifest.manifestRef),
    operations: [operation],
  })
}

const acceptProposal = async (
  fixture: Awaited<ReturnType<typeof setup>>,
  proposal: ReturnType<typeof proposalForWorkspace>,
) => {
  expect((await fixture.host.command({
    _tag: "SubmitProposal",
    input: { proposal, expectedAttachmentGeneration: fixture.attachment.attachmentGeneration },
  }))._tag).toBe("Succeeded")
  expect((await fixture.host.command({
    _tag: "BeginReview",
    input: {
      proposalRef: proposal.proposalRef,
      reviewRef: IdeAgentReviewRefSchema.make(`ide.agent-review.${suffix(proposal.proposalRef)}`),
      expectedAttachmentGeneration: fixture.attachment.attachmentGeneration,
    },
  }))._tag).toBe("Succeeded")
  return fixture.host.command({
    _tag: "Decide",
    decision: IdeAgentDecisionSchema.make({
      decisionRef: IdeAgentDecisionRefSchema.make(`ide.agent-decision.${suffix(proposal.proposalRef)}`),
      proposalRef: proposal.proposalRef,
      decidedAt: IdeTimestampSchema.make("2026-07-19T15:00:00.000Z"),
      disposition: "accept",
      operationRefs: proposal.operations.map(operation => operation.operationRef),
      reason: null,
    }),
    expectedAttachmentGeneration: fixture.attachment.attachmentGeneration,
  })
}

describe("IDE-08 main-owned agent-code host", () => {
  test("refuses an attachment transition that does not match current portable authority", async () => {
    const portable = makePortableAuthority(2)
    const fixture = await setup({ mutationAuthority: portable.authority })

    expect(await fixture.host.command({ _tag: "Attach", attachment: fixture.attachment })).toMatchObject({
      _tag: "Refused",
      reason: "grant_revoked",
    })
    expect((await fixture.host.snapshot()).attachment).toBeNull()
  })

  test("keeps proposal review readable while stale authority refuses Apply", async () => {
    const portable = makePortableAuthority()
    const fixture = await setup({ mutationAuthority: portable.authority })
    await admit(fixture)
    portable.setGeneration(2)

    const proposal = proposalForWorkspace(fixture)
    const accepted = await acceptProposal(fixture, proposal)
    expect(accepted._tag).toBe("Succeeded")
    if (accepted._tag !== "Succeeded") return
    expect(accepted.snapshot.proposals.at(-1)?.lifecycle._tag).toBe("Accepted")

    const result = await fixture.host.command({
      _tag: "Apply",
      input: {
        proposalRef: proposal.proposalRef,
        operationRefs: proposal.operations.map(operation => operation.operationRef),
        expectedAttachmentGeneration: fixture.attachment.attachmentGeneration,
        expectedProposalRevision: accepted.snapshot.revision,
      },
    })
    expect(result).toMatchObject({ _tag: "Refused", reason: "grant_revoked" })
    expect(result.snapshot.proposals.at(-1)?.lifecycle._tag).toBe("Accepted")
    expect(readFileSync(path.join(fixture.root, "app.ts"), "utf8")).toBe("export const answer = 41\n")
  })

  test("drops a late Applied receipt and persisted snapshot after attachment generation changes", async () => {
    const portable = makePortableAuthority()
    let revokeOnReplace = false
    const documentIo: WorkspaceDocumentIo = {
      read: absolutePath => readFileSync(absolutePath),
      replace: (absolutePath, bytes) => {
        writeFileSync(absolutePath, bytes)
        if (revokeOnReplace) portable.setGeneration(portable.generation() + 1)
      },
      create: (absolutePath, bytes) => writeFileSync(absolutePath, bytes, { flag: "wx" }),
    }
    const fixture = await setup({ mutationAuthority: portable.authority, documentIo })
    await admit(fixture)
    const proposal = proposalForWorkspace(fixture)
    const accepted = await acceptProposal(fixture, proposal)
    expect(accepted._tag).toBe("Succeeded")
    if (accepted._tag !== "Succeeded") return

    revokeOnReplace = true
    const result = await fixture.host.command({
      _tag: "Apply",
      input: {
        proposalRef: proposal.proposalRef,
        operationRefs: proposal.operations.map(operation => operation.operationRef),
        expectedAttachmentGeneration: fixture.attachment.attachmentGeneration,
        expectedProposalRevision: accepted.snapshot.revision,
      },
    })
    expect(result).toMatchObject({ _tag: "Refused", reason: "grant_revoked" })
    expect(result.snapshot.lifecycle).toBe("stopped")
    expect(result.snapshot.applyReceipts).toHaveLength(0)

    const persisted = decodeIdeAgentCodeSnapshot(JSON.parse(readFileSync(fixture.persistencePath, "utf8")))
    expect(persisted?.proposals.at(-1)?.lifecycle._tag).toBe("Accepted")
    expect(persisted?.applyReceipts).toHaveLength(0)
  })

  test("drops a late Undone receipt after attachment generation changes", async () => {
    const portable = makePortableAuthority()
    let revokeOnReplace = false
    const documentIo: WorkspaceDocumentIo = {
      read: absolutePath => readFileSync(absolutePath),
      replace: (absolutePath, bytes) => {
        writeFileSync(absolutePath, bytes)
        if (revokeOnReplace) portable.setGeneration(portable.generation() + 1)
      },
      create: (absolutePath, bytes) => writeFileSync(absolutePath, bytes, { flag: "wx" }),
    }
    const fixture = await setup({ mutationAuthority: portable.authority, documentIo })
    await admit(fixture)
    const proposal = proposalForWorkspace(fixture)
    const accepted = await acceptProposal(fixture, proposal)
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
    if (applied._tag !== "Succeeded") return
    const lifecycle = applied.snapshot.proposals.at(-1)?.lifecycle
    expect(lifecycle?._tag).toBe("Applied")
    if (lifecycle?._tag !== "Applied") return

    revokeOnReplace = true
    const result = await fixture.host.command({
      _tag: "Undo",
      input: {
        proposalRef: proposal.proposalRef,
        applyRef: lifecycle.applyRef,
        checkpointRef: lifecycle.checkpointRef,
        expectedAttachmentGeneration: fixture.attachment.attachmentGeneration,
      },
    })
    expect(result).toMatchObject({ _tag: "Refused", reason: "grant_revoked" })
    expect(result.snapshot.undoReceipts).toHaveLength(0)

    const persisted = decodeIdeAgentCodeSnapshot(JSON.parse(readFileSync(fixture.persistencePath, "utf8")))
    expect(persisted?.proposals.at(-1)?.lifecycle._tag).toBe("Applied")
    expect(persisted?.undoReceipts).toHaveLength(0)
  })

  test("keeps a partial lower-layer failure visibly rollback-failed", async () => {
    let replacementCount = 0
    const documentIo: WorkspaceDocumentIo = {
      read: absolutePath => readFileSync(absolutePath),
      replace: (absolutePath, bytes) => {
        replacementCount += 1
        if (replacementCount > 1) throw new Error("fixture replacement failure")
        writeFileSync(absolutePath, bytes)
      },
      create: (absolutePath, bytes) => writeFileSync(absolutePath, bytes, { flag: "wx" }),
    }
    const fixture = await setup({ documentIo })
    writeFileSync(path.join(fixture.root, "second.ts"), "export const second = 1\n")
    await admit(fixture)
    const proposal = IdeAgentProposalSchema.make({
      ...ideAgentFixtureProposal(),
      proposalRef: IdeProposalRefSchema.make("ide.proposal.workspace-host.rollback-failed"),
      attachment: fixture.attachment,
      manifestRef: IdeAgentManifestRefSchema.make(fixture.manifest.manifestRef),
      operations: [
        editOperationForWorkspace(fixture, "app.ts", "export const answer = 42\n", "rollback-first"),
        editOperationForWorkspace(fixture, "second.ts", "export const second = 2\n", "rollback-second"),
      ],
    })
    const accepted = await acceptProposal(fixture, proposal)
    expect(accepted._tag).toBe("Succeeded")
    if (accepted._tag !== "Succeeded") return

    const result = await fixture.host.command({
      _tag: "Apply",
      input: {
        proposalRef: proposal.proposalRef,
        operationRefs: proposal.operations.map(operation => operation.operationRef),
        expectedAttachmentGeneration: fixture.attachment.attachmentGeneration,
        expectedProposalRevision: accepted.snapshot.revision,
      },
    })
    expect(result).toMatchObject({ _tag: "Refused", reason: "rollback_failed" })
    expect(result.snapshot.proposals.at(-1)?.lifecycle).toMatchObject({
      _tag: "Failed",
      recoverable: false,
    })
    await fixture.host.dispose()
    const persisted = decodeIdeAgentCodeSnapshot(JSON.parse(readFileSync(fixture.persistencePath, "utf8")))
    expect(persisted?.proposals.at(-1)?.lifecycle._tag).toBe("Failed")
  })

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
