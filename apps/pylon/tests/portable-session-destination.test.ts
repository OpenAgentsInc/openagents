import { Runtime } from "@openagentsinc/runtime-platform"
import { NodeTestDatabase } from "@openagentsinc/sqlite-runtime/test"
import { afterEach, describe, expect, test } from "vite-plus/test"
import { createHash } from "node:crypto"
import { access, cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { canonicalJson } from "@openagentsinc/khala-sync"
import { Effect } from "effect"

import {
  createPylonOwnerLocalDestinationLifecycle,
  type PylonPortableAuthorityAttachment,
  type PylonPortableLocalRehydrator,
  type PylonPortableLocalStage,
} from "../src/portable-session-destination.js"
import { PylonPortableCheckpointArtifactStore } from "../src/portable-session-checkpoint-artifact.js"
import { createPylonPortableLocalRehydrator } from "../src/portable-session-local-rehydrator.js"
import {
  type PylonPortableCheckpointBundle,
  PylonPortableSessionOperationLedger,
} from "../src/portable-session-operation-ledger.js"

const roots: string[] = []

const unsupportedHelpers = () => (["pty", "lsp", "dap", "watcher", "native"] as const).map(kind => ({
  kind,
  readiness: "unsupported" as const,
  instanceRef: null,
  versionRef: null,
  omissionRef: `omission.pylon.portable.${kind}.unsupported`,
  evidenceRefs: [],
}))

afterEach(async () => {
  await Promise.all(roots.splice(0).map(root => rm(root, { recursive: true, force: true })))
})

test("concrete rehydrator restores private repository bytes and activates the retained graph", async () => {
  const root = await mkdtemp(join(tmpdir(), "oa-port03-concrete-rehydrate-"))
  roots.push(root)
  const { bundle, repository } = await fixture(root)
  const artifacts = new PylonPortableCheckpointArtifactStore()
  artifacts.register({ bundle, workingDirectory: repository })
  let stagedDirectory = ""
  let stagedWorkspaceRef = ""
  const runnerReservationRef = "runner-session-reservation.port03.concrete"
  const observedRunnerReservationRefs: string[] = []
  const lifecycle = {
    bind: () => undefined,
    quiesce: async () => ({ quiescedAgentRefs: [], evidenceRefs: [] }),
    checkpointSource: async () => ({ workingDirectory: repository, workspaceRef: "workspace.test", artifactRefs: [], approvalRefs: [] }),
    cleanup: async () => ({ cleanedAgentRefs: [], cleanupReceiptRef: "receipt.test.cleanup", evidenceRefs: [] }),
    stageDestination: async (input: { workingDirectory: string; workspaceRef: string; destinationRunnerSessionReservationRef: string }) => {
      stagedDirectory = input.workingDirectory
      stagedWorkspaceRef = input.workspaceRef
      observedRunnerReservationRefs.push(input.destinationRunnerSessionReservationRef)
      return { evidenceRefs: ["receipt.port03.concrete.staged"] }
    },
    activateDestination: async (input: { workingDirectory: string; workspaceRef: string; authorityEvidenceRef: string; authenticationPolicyRef: string; destinationRunnerSessionReservationRef: string }) => {
      expect(input.workingDirectory).toBe(stagedDirectory)
      expect(input.workspaceRef).toBe(stagedWorkspaceRef)
      expect(input.destinationRunnerSessionReservationRef).toBe(runnerReservationRef)
      return {
        authentication: {
          state: "reauthenticated" as const,
          policyRef: input.authenticationPolicyRef,
          evidenceRef: input.authorityEvidenceRef,
          observedAt: "2026-07-20T08:00:00.000Z",
          expiresAt: null,
        },
        helpersObservedAt: new Date().toISOString(),
        helpers: unsupportedHelpers(),
        // The production helper supervisor includes the authentication evidence.
        // The rehydrator must preserve it exactly once when it adds authority evidence.
        evidenceRefs: [input.authorityEvidenceRef, "receipt.port03.concrete.activated"],
      }
    },
    abortDestination: async () => ({ evidenceRefs: ["receipt.port03.concrete.aborted"] }),
  }
  const rehydrator = createPylonPortableLocalRehydrator({
    targetRef: "target.port03.owner.local",
    custodyRoot: join(root, "local-rehydration"),
    artifacts,
    lifecycle,
  })
  const stage = await rehydrator.stage({
    operationRef: "operation.port03.concrete.destination.stage",
    destinationRunnerSessionReservationRef: runnerReservationRef,
    bundle,
    destinationAttachmentRef: "attachment.port03.local.3",
    destinationGeneration: 3,
    capabilityLeaseRefs: ["lease.port03.local.3"],
  })
  expect(await readFile(join(stagedDirectory, "tracked.txt"), "utf8")).toBe("changed\n")
  expect(await readFile(join(stagedDirectory, "untracked.txt"), "utf8")).toBe("new\n")
  expect(await rehydrator.readStage(stage.operationRef)).toEqual(stage)
  expect(stage.destinationRunnerSessionReservationRef).toBe(runnerReservationRef)
  await expect(rehydrator.stage({
    operationRef: stage.operationRef,
    destinationRunnerSessionReservationRef: "runner-session-reservation.port03.conflict",
    bundle,
    destinationAttachmentRef: stage.destinationAttachmentRef,
    destinationGeneration: stage.destinationGeneration,
    capabilityLeaseRefs: stage.capabilityLeaseRefs,
  })).rejects.toThrow("conflicts with persisted stage")
  await expect(rehydrator.activate({
    operationRef: "operation.port03.concrete.destination.activate.conflict",
    destinationRunnerSessionReservationRef: "runner-session-reservation.port03.conflict",
    stage,
    authorityEvidenceRef: "authority.port03.local.3",
    executionBinding: bundle.executionBinding,
  })).rejects.toThrow("does not match its stage")
  expect(await rehydrator.activate({
    operationRef: "operation.port03.concrete.destination.activate",
    destinationRunnerSessionReservationRef: runnerReservationRef,
    stage,
    authorityEvidenceRef: "authority.port03.local.3",
    executionBinding: bundle.executionBinding,
  })).toMatchObject({
    schema: "openagents.ide_portable_destination_activation.v1",
    operationRef: "operation.port03.concrete.destination.activate",
    sessionRef: bundle.checkpoint.sessionRef,
    checkpointRef: bundle.checkpoint.checkpointRef,
    destinationTargetRef: "target.port03.owner.local",
    destinationAttachmentRef: stage.destinationAttachmentRef,
    destinationGeneration: 3,
    authentication: { state: "reauthenticated", policyRef: "policy.portable.destination.owner_local.v1" },
    helpers: unsupportedHelpers(),
    activatedAgentRefs: bundle.graph.nodes.map(node => node.agentRef),
    acceptedWorkRefs: [],
    evidenceRefs: ["authority.port03.local.3", "receipt.port03.concrete.activated"],
  })
  expect(observedRunnerReservationRefs).toEqual([runnerReservationRef, runnerReservationRef])
})

test("concrete rehydrator cleans a partial destination when helper readiness is invalid", async () => {
  const root = await mkdtemp(join(tmpdir(), "oa-port03-readiness-cleanup-"))
  roots.push(root)
  const { bundle, repository } = await fixture(root)
  const artifacts = new PylonPortableCheckpointArtifactStore()
  artifacts.register({ bundle, workingDirectory: repository })
  let stagedDirectory = ""
  let aborts = 0
  const lifecycle = {
    bind: () => undefined,
    quiesce: async () => ({ quiescedAgentRefs: [], evidenceRefs: [] }),
    checkpointSource: async () => ({ workingDirectory: repository, workspaceRef: "workspace.test", artifactRefs: [], approvalRefs: [] }),
    cleanup: async () => ({ cleanedAgentRefs: [], cleanupReceiptRef: "receipt.test.cleanup", evidenceRefs: [] }),
    stageDestination: async (input: { workingDirectory: string }) => {
      stagedDirectory = input.workingDirectory
      return { evidenceRefs: ["receipt.destination.staged"] }
    },
    activateDestination: async (input: { authorityEvidenceRef: string; authenticationPolicyRef: string }) => ({
      authentication: {
        state: "reauthenticated" as const,
        policyRef: input.authenticationPolicyRef,
        evidenceRef: input.authorityEvidenceRef,
        observedAt: "2026-07-20T08:00:00.000Z",
        expiresAt: null,
      },
      helpersObservedAt: new Date().toISOString(),
      helpers: unsupportedHelpers().slice(1),
      evidenceRefs: ["receipt.destination.partial_start"],
    }),
    abortDestination: async () => {
      aborts += 1
      return { evidenceRefs: ["receipt.destination.partial_cleaned"] }
    },
  }
  const rehydrator = createPylonPortableLocalRehydrator({
    targetRef: "target.port03.owner.local",
    custodyRoot: join(root, "local-rehydration"),
    artifacts,
    lifecycle,
  })
  const stage = await rehydrator.stage({
    operationRef: "operation.port03.cleanup.destination.stage",
    destinationRunnerSessionReservationRef: "runner-session-reservation.port03.cleanup",
    bundle,
    destinationAttachmentRef: "attachment.port03.cleanup.3",
    destinationGeneration: 3,
    capabilityLeaseRefs: ["lease.port03.cleanup.3"],
  })
  await expect(rehydrator.activate({
    operationRef: "operation.port03.cleanup.destination.activate",
    destinationRunnerSessionReservationRef: stage.destinationRunnerSessionReservationRef,
    stage,
    authorityEvidenceRef: "authority.port03.cleanup.3",
    executionBinding: bundle.executionBinding,
  })).rejects.toThrow("helper inventory")
  expect(aborts).toBe(1)
  await expect(access(stagedDirectory)).rejects.toThrow()
})

const git = async (cwd: string, ...args: string[]): Promise<Uint8Array> => {
  const proc = Runtime.spawn(["git", ...args], { cwd, stdout: "pipe", stderr: "pipe" })
  const [stdout, stderr, code] = await Promise.all([
    new Response(proc.stdout).bytes(),
    new Response(proc.stderr).text(),
    proc.exited,
  ])
  if (code !== 0) throw new Error(stderr)
  return stdout
}

const digest = (value: string | Uint8Array): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`

const repositoryDigests = async (cwd: string) => {
  const listed = new TextDecoder().decode(await git(cwd, "ls-files", "-co", "--exclude-standard", "-z"))
    .split("\0").filter(Boolean).sort()
  const postImage = createHash("sha256")
  for (const relativePath of listed) {
    postImage.update(relativePath).update("\0").update(await readFile(join(cwd, relativePath))).update("\0")
  }
  const diffHash = createHash("sha256").update(await git(cwd, "diff", "--binary", "HEAD", "--"))
  const untracked = new TextDecoder().decode(await git(cwd, "ls-files", "--others", "--exclude-standard", "-z"))
    .split("\0").filter(Boolean).sort()
  for (const relativePath of untracked) {
    diffHash.update(relativePath).update("\0").update(await readFile(join(cwd, relativePath))).update("\0")
  }
  return {
    repositoryPostImageDigest: `sha256:${postImage.digest("hex")}` as const,
    diffDigest: `sha256:${diffHash.digest("hex")}` as const,
  }
}

const fixture = async (root: string): Promise<{ bundle: PylonPortableCheckpointBundle; repository: string }> => {
  const repository = join(root, "source")
  await mkdir(repository, { recursive: true })
  await git(repository, "init", "-b", "main")
  await git(repository, "config", "user.email", "test@openagents.com")
  await git(repository, "config", "user.name", "OpenAgents Test")
  await writeFile(join(repository, "tracked.txt"), "base\n", "utf8")
  await git(repository, "add", "tracked.txt")
  await git(repository, "commit", "-m", "base")
  await writeFile(join(repository, "tracked.txt"), "changed\n", "utf8")
  await writeFile(join(repository, "untracked.txt"), "new\n", "utf8")
  const revision = new TextDecoder().decode(await git(repository, "rev-parse", "HEAD")).trim()
  const repositoryState = await repositoryDigests(repository)
  const graph = {
    rootAgentRef: "agent.port03.local.root",
    nodes: [
      {
        agentRef: "agent.port03.local.root",
        threadRef: "thread.port03.local.root",
        transcriptRef: "transcript.port03.local.root",
        activityCursor: 12,
        lifecycle: "quiesced" as const,
        attachmentGeneration: 2,
      },
      {
        agentRef: "agent.port03.local.child",
        parentAgentRef: "agent.port03.local.root",
        threadRef: "thread.port03.local.child",
        transcriptRef: "transcript.port03.local.child",
        activityCursor: 7,
        lifecycle: "quiesced" as const,
        attachmentGeneration: 2,
      },
    ],
  }
  const graphDigest = digest(canonicalJson({
    rootAgentRef: graph.rootAgentRef,
    nodes: [...graph.nodes].sort((left, right) => left.agentRef.localeCompare(right.agentRef)),
  }))
  const payload = {
    schema: "openagents.portable_checkpoint.v1" as const,
    checkpointRef: "checkpoint.port03.failback.2",
    sessionRef: "session.port03.failback",
    sourceAttachmentRef: "attachment.port03.managed.2",
    sourceGeneration: 2,
    repositoryRef: "repository.port03.fixture",
    repositoryRevisionRef: revision,
    ...repositoryState,
    eventLogCursor: 19,
    catalogGenerationRef: "catalog.port03.generation.2",
    graphDigest,
    approvalRefs: ["approval.port03.exact"],
    artifactRefs: ["artifact.port03.exact"],
    receiptRefs: ["receipt.port03.managed.checkpoint"],
    secretMaterial: "excluded" as const,
    processState: "excluded" as const,
  }
  return {
    repository,
    bundle: {
      checkpoint: { ...payload, digest: digest(canonicalJson(payload)) },
      executionBinding: {
        schema: "openagents.portable_session_execution_binding.v1",
        sessionRef: payload.sessionRef,
        ownerRef: "owner.port03.fixture",
        runRef: "run.port03.fixture",
        repositoryRef: payload.repositoryRef,
        pinnedBaseRef: `commit.${revision}`,
      },
      graph,
      threadCursors: graph.nodes.map((node, index) => ({
        threadRef: node.threadRef,
        transcriptRef: node.transcriptRef,
        activityCursor: node.activityCursor,
        eventCursor: 20 + index,
      })),
    },
  }
}

const createRehydrator = (input: Readonly<{
  root: string
  repository: string
  corruptDiff?: boolean
}>) => {
  const stages = new Map<string, PylonPortableLocalStage>()
  const activationResults = new Map<string, Awaited<ReturnType<PylonPortableLocalRehydrator["activate"]>>>()
  const abortedOperations = new Set<string>()
  const activatedRunnerReservationRefs: string[] = []
  let stageEffects = 0
  let activationEffects = 0
  let abortEffects = 0
  let accepting = false
  const stageDirectory = (operationRef: string) => join(input.root, "rehydrated", operationRef.replaceAll(".", "-"))
  const rehydrator: PylonPortableLocalRehydrator = {
    stage: async operation => {
      const replay = stages.get(operation.operationRef)
      if (replay !== undefined) return replay
      stageEffects += 1
      const directory = stageDirectory(operation.operationRef)
      await mkdir(join(input.root, "rehydrated"), { recursive: true })
      await cp(input.repository, directory, { recursive: true })
      const observed = await repositoryDigests(directory)
      const stage: PylonPortableLocalStage = {
        operationRef: operation.operationRef,
        destinationRunnerSessionReservationRef: operation.destinationRunnerSessionReservationRef,
        sessionRef: operation.bundle.checkpoint.sessionRef,
        checkpointRef: operation.bundle.checkpoint.checkpointRef,
        checkpointDigest: operation.bundle.checkpoint.digest,
        sourceAttachmentRef: operation.bundle.checkpoint.sourceAttachmentRef,
        sourceGeneration: operation.bundle.checkpoint.sourceGeneration,
        destinationAttachmentRef: operation.destinationAttachmentRef,
        destinationGeneration: operation.destinationGeneration,
        repositoryPostImageDigest: observed.repositoryPostImageDigest,
        diffDigest: input.corruptDiff ? `sha256:${"0".repeat(64)}` : observed.diffDigest,
        graphDigest: operation.bundle.checkpoint.graphDigest,
        stagedAgentRefs: operation.bundle.graph.nodes.map(node => node.agentRef),
        threadCursors: operation.bundle.threadCursors,
        capabilityLeaseRefs: operation.capabilityLeaseRefs,
        acceptingWork: false,
        evidenceRefs: ["receipt.port03.local.stage"],
      }
      stages.set(operation.operationRef, stage)
      return stage
    },
    readStage: async operationRef => {
      const stage = stages.get(operationRef)
      if (stage === undefined) throw new Error("stage absent")
      return stage
    },
    activate: async operation => {
      const replay = activationResults.get(operation.operationRef)
      if (replay !== undefined) return replay
      activationEffects += 1
      activatedRunnerReservationRefs.push(operation.destinationRunnerSessionReservationRef)
      accepting = true
      const result = {
        schema: "openagents.ide_portable_destination_activation.v1" as const,
        receiptRef: `receipt.${operation.operationRef}`,
        operationRef: operation.operationRef,
        sessionRef: operation.stage.sessionRef,
        checkpointRef: operation.stage.checkpointRef,
        destinationTargetRef: "target.port03.owner.local",
        destinationAttachmentRef: operation.stage.destinationAttachmentRef,
        destinationRunnerSessionReservationRef:
          operation.destinationRunnerSessionReservationRef,
        destinationGeneration: operation.stage.destinationGeneration,
        authentication: {
          state: "reauthenticated" as const,
          policyRef: "policy.portable.destination.owner_local.v1",
          evidenceRef: operation.authorityEvidenceRef,
          observedAt: "2026-07-20T08:00:00.000Z",
          expiresAt: null,
        },
        helpersObservedAt: new Date().toISOString(),
        helpers: unsupportedHelpers(),
        activatedAgentRefs: operation.stage.stagedAgentRefs,
        acceptedWorkRefs: [],
        evidenceRefs: [operation.authorityEvidenceRef, "receipt.port03.local.activated"],
      }
      activationResults.set(operation.operationRef, result)
      return result
    },
    abort: async operation => {
      if (!abortedOperations.has(operation.operationRef)) {
        abortedOperations.add(operation.operationRef)
        abortEffects += 1
        await rm(stageDirectory(operation.stage.operationRef), { recursive: true, force: true })
      }
      return {
        cleanedAgentRefs: operation.stage.stagedAgentRefs,
        releasedCapabilityLeaseRefs: operation.stage.capabilityLeaseRefs,
        processes: "released",
        scratch: "released",
        ports: "released",
        evidenceRefs: ["receipt.port03.local.abort.cleaned"],
      }
    },
  }
  return {
    rehydrator,
    stageDirectory,
    counters: () => ({ stageEffects, activationEffects, abortEffects }),
    acceptsWork: () => accepting,
    activatedRunnerReservationRefs: () => [...activatedRunnerReservationRefs],
  }
}

describe("owner-local portable destination rehydration", () => {
  test("restores an exact child-bearing repository graph, fences replies, and replays activation after restart", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-port03-local-destination-"))
    roots.push(root)
    const { bundle, repository } = await fixture(root)
    const databasePath = join(root, "portable.sqlite")
    const database = new NodeTestDatabase(databasePath, { create: true })
    const ledger = new PylonPortableSessionOperationLedger(database)
    await Effect.runPromise(ledger.registerSession({
      sessionRef: bundle.checkpoint.sessionRef,
      attachmentRef: "attachment.port03.local.1",
      generation: 1,
      acceptingWork: false,
    }))
    let authority: PylonPortableAuthorityAttachment = {
      sessionRef: bundle.checkpoint.sessionRef,
      targetRef: "target.port03.managed",
      attachmentRef: bundle.checkpoint.sourceAttachmentRef,
      generation: bundle.checkpoint.sourceGeneration,
      state: "active",
      checkpointRef: bundle.checkpoint.checkpointRef,
      authorityEvidenceRef: "authority.port03.managed.2",
    }
    const runtime = createRehydrator({ root, repository })
    const lifecycle = createPylonOwnerLocalDestinationLifecycle({
      targetRef: "target.port03.owner.local",
      ledger,
      authority: { readCurrentAttachment: async () => authority },
      rehydrator: runtime.rehydrator,
    })
    const stageInput = {
      operationRef: "operation.port03.failback.destination.stage",
      bundle,
      destinationAttachmentRef: "attachment.port03.local.3",
      destinationGeneration: 3,
      capabilityLeaseRefs: ["lease.port03.provider.local.3", "lease.port03.scm.local.3"],
    }
    const staged = await lifecycle.stageCheckpoint(stageInput)
    expect(staged).toMatchObject({
      destinationRunnerSessionReservationRef: expect.stringMatching(
        /^runner-session-reservation\.[A-Za-z0-9-]+$/u,
      ),
      checkpointDigest: bundle.checkpoint.digest,
      repositoryPostImageDigest: bundle.checkpoint.repositoryPostImageDigest,
      diffDigest: bundle.checkpoint.diffDigest,
      graphDigest: bundle.checkpoint.graphDigest,
      acceptingWork: false,
    })
    expect(runtime.acceptsWork()).toBe(false)
    expect((await Effect.runPromise(ledger.readSession(bundle.checkpoint.sessionRef))).acceptingWork).toBe(false)
    expect(await lifecycle.stageCheckpoint(stageInput)).toEqual(staged)
    expect(runtime.counters().stageEffects).toBe(1)

    const activationInput = {
      operationRef: "operation.port03.failback.destination.activate",
      checkpointRef: bundle.checkpoint.checkpointRef,
      sessionRef: bundle.checkpoint.sessionRef,
      executionBinding: bundle.executionBinding,
      destinationAttachmentRef: stageInput.destinationAttachmentRef,
      destinationGeneration: stageInput.destinationGeneration,
      capabilityLeaseRefs: stageInput.capabilityLeaseRefs,
    }
    await expect(lifecycle.activate(activationInput)).rejects.toMatchObject({ reason: "authority_mismatch" })
    expect(runtime.acceptsWork()).toBe(false)
    authority = {
      ...authority,
      targetRef: "target.port03.owner.local",
      attachmentRef: stageInput.destinationAttachmentRef,
      generation: stageInput.destinationGeneration,
      checkpointRef: bundle.checkpoint.checkpointRef,
      authorityEvidenceRef: "authority.port03.local.3",
    }
    const activated = await lifecycle.activate(activationInput)
    expect([...activated.activatedAgentRefs].sort()).toEqual(bundle.graph.nodes.map(node => node.agentRef).sort())
    expect(activated.evidenceRefs.filter(ref => ref === authority.authorityEvidenceRef)).toHaveLength(1)
    expect(runtime.acceptsWork()).toBe(true)
    expect(runtime.activatedRunnerReservationRefs()).toEqual([
      staged.destinationRunnerSessionReservationRef,
    ])
    expect(await Effect.runPromise(ledger.readSession(bundle.checkpoint.sessionRef))).toMatchObject({
      attachmentRef: stageInput.destinationAttachmentRef,
      generation: 3,
      acceptingWork: true,
    })
    database.close()

    const reopened = new NodeTestDatabase(databasePath)
    const restartedLedger = new PylonPortableSessionOperationLedger(reopened)
    const restarted = createPylonOwnerLocalDestinationLifecycle({
      targetRef: "target.port03.owner.local",
      ledger: restartedLedger,
      authority: { readCurrentAttachment: async () => authority },
      rehydrator: runtime.rehydrator,
    })
    expect(await restarted.stageCheckpoint(stageInput)).toEqual(staged)
    expect(runtime.counters().stageEffects).toBe(1)
    expect(await restarted.activate(activationInput)).toEqual(activated)
    expect(runtime.counters()).toEqual({ stageEffects: 1, activationEffects: 1, abortEffects: 0 })
    expect(runtime.activatedRunnerReservationRefs()).toEqual([
      staged.destinationRunnerSessionReservationRef,
    ])
    reopened.close()
  })

  test("rejects a mismatched restored diff and aborts an exact stage with zero residue", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-port03-local-abort-"))
    roots.push(root)
    const { bundle, repository } = await fixture(root)
    const database = new NodeTestDatabase(join(root, "portable.sqlite"), { create: true })
    const ledger = new PylonPortableSessionOperationLedger(database)
    await Effect.runPromise(ledger.registerSession({
      sessionRef: bundle.checkpoint.sessionRef,
      attachmentRef: "attachment.port03.local.1",
      generation: 1,
      acceptingWork: false,
    }))
    const authority: PylonPortableAuthorityAttachment = {
      sessionRef: bundle.checkpoint.sessionRef,
      targetRef: "target.port03.managed",
      attachmentRef: bundle.checkpoint.sourceAttachmentRef,
      generation: 2,
      state: "active",
      checkpointRef: bundle.checkpoint.checkpointRef,
      authorityEvidenceRef: "authority.port03.managed.2",
    }
    const badRuntime = createRehydrator({ root: join(root, "bad"), repository, corruptDiff: true })
    const bad = createPylonOwnerLocalDestinationLifecycle({
      targetRef: "target.port03.owner.local",
      ledger,
      authority: { readCurrentAttachment: async () => authority },
      rehydrator: badRuntime.rehydrator,
    })
    const badStage = {
      operationRef: "operation.port03.bad.destination.stage",
      bundle,
      destinationAttachmentRef: "attachment.port03.local.bad.3",
      destinationGeneration: 3,
      capabilityLeaseRefs: ["lease.port03.local.bad.3"],
    }
    await expect(bad.stageCheckpoint(badStage)).rejects.toMatchObject({ reason: "rehydration_failed" })
    await bad.abortStaged({
      operationRef: "operation.port03.bad.destination.abort",
      sessionRef: bundle.checkpoint.sessionRef,
      destinationAttachmentRef: badStage.destinationAttachmentRef,
      destinationGeneration: badStage.destinationGeneration,
    })
    await expect(access(badRuntime.stageDirectory(badStage.operationRef))).rejects.toThrow()

    const goodRuntime = createRehydrator({ root: join(root, "good"), repository })
    const good = createPylonOwnerLocalDestinationLifecycle({
      targetRef: "target.port03.owner.local",
      ledger,
      authority: { readCurrentAttachment: async () => authority },
      rehydrator: goodRuntime.rehydrator,
    })
    const stageInput = {
      operationRef: "operation.port03.abort.destination.stage",
      bundle,
      destinationAttachmentRef: "attachment.port03.local.abort.3",
      destinationGeneration: 3,
      capabilityLeaseRefs: ["lease.port03.provider.abort.3"],
    }
    await good.stageCheckpoint(stageInput)
    const aborted = await good.abortStaged({
      operationRef: "operation.port03.abort.destination.abort",
      sessionRef: bundle.checkpoint.sessionRef,
      destinationAttachmentRef: stageInput.destinationAttachmentRef,
      destinationGeneration: stageInput.destinationGeneration,
    })
    expect(aborted.evidenceRefs).toEqual(["receipt.port03.local.abort.cleaned"])
    await expect(access(goodRuntime.stageDirectory(stageInput.operationRef))).rejects.toThrow()
    expect(await good.abortStaged({
      operationRef: "operation.port03.abort.destination.abort",
      sessionRef: bundle.checkpoint.sessionRef,
      destinationAttachmentRef: stageInput.destinationAttachmentRef,
      destinationGeneration: stageInput.destinationGeneration,
    })).toEqual(aborted)
    expect(goodRuntime.counters().abortEffects).toBe(1)
    expect(await Effect.runPromise(ledger.readSession(bundle.checkpoint.sessionRef))).toMatchObject({
      attachmentRef: "attachment.port03.local.1",
      generation: 1,
      acceptingWork: false,
    })
    database.close()
  })
})
