import { createHash } from "node:crypto"
import { cp, mkdir, mkdtemp, readFile, readlink, rm, symlink, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { afterEach, describe, expect, test } from "bun:test"

import {
  continuePortableSession,
  executePortableSessionControl,
  exportPortableCheckpoint,
  installPortableCapability,
  materializePortableCheckpoint,
  portableSessionRoot,
  productionRuntime,
  repositorySnapshot,
  PortableSessionControlError,
  type PortableSessionGuestRuntime,
} from "../deploy/agent-computer/portable-session-control.js"

const roots: string[] = []
afterEach(async () => Promise.all(roots.splice(0).map(root => rm(root, { recursive: true, force: true }))))

const operation = (action: string, operationRef: string, payload: Record<string, unknown>) => ({
  operationRef,
  action,
  ownerRef: "owner.port03.guest",
  targetRef: "target.port03.guest.managed",
  sessionRef: "session.port03.guest",
  attachmentRef: "attachment.port03.guest.managed",
  generation: 2,
  resourceRef: "resource.port03.guest",
  payload,
})

const graph = {
  rootAgentRef: "agent.port03.guest.root",
  nodes: [
    { agentRef: "agent.port03.guest.root" },
    { agentRef: "agent.port03.guest.child", parentAgentRef: "agent.port03.guest.root" },
  ],
}

const bundle = {
  checkpoint: {
    sessionRef: "session.port03.guest",
    sourceGeneration: 1,
    digest: `sha256:${"d".repeat(64)}`,
    repositoryRevisionRef: "a".repeat(40),
    repositoryPostImageDigest: `sha256:${"a".repeat(64)}`,
    diffDigest: `sha256:${"b".repeat(64)}`,
    graphDigest: `sha256:${"c".repeat(64)}`,
  },
  executionBinding: { runRef: "run.port03.guest" },
  graph,
  threadCursors: [{
    threadRef: "thread.port03.guest.root",
    transcriptRef: "transcript.port03.guest.root",
    activityCursor: 3,
    eventCursor: 8,
  }],
}

const fixture = async () => {
  const stateRoot = await mkdtemp(join(tmpdir(), "portable-session-control-"))
  roots.push(stateRoot)
  const calls: string[] = []
  const runtime: PortableSessionGuestRuntime = {
    prepare: async () => { calls.push("prepare") },
    verifyStage: async () => { calls.push("stage") },
    verifyCapabilities: async () => { calls.push("capabilities") },
    activate: async () => { calls.push("activate") },
    continueWork: async ({ turns }) => {
      calls.push("continue")
      return turns.map((turn, index) => ({
        agentRef: turn.agentRef,
        turnRef: turn.turnRef,
        activityCursor: turn.activityCursor + 1,
        eventCursor: turn.eventCursor + 1,
      }))
    },
    quiesce: async () => { calls.push("quiesce") },
    reclaim: async () => { calls.push("reclaim") },
    repositorySnapshot: async () => ({
      repositoryRevisionRef: "e".repeat(40),
      repositoryPostImageDigest: `sha256:${"e".repeat(64)}`,
      diffDigest: `sha256:${"f".repeat(64)}`,
    }),
  }
  return { stateRoot, calls, runtime }
}

const run = async (command: ReadonlyArray<string>, cwd?: string): Promise<void> => {
  const child = Bun.spawn(command, { cwd, stdout: "pipe", stderr: "pipe", env: { ...process.env, COPYFILE_DISABLE: "1" } })
  if (await child.exited !== 0) throw new Error(await new Response(child.stderr).text())
}

const canonical = (value: unknown): string => {
  if (value === null || typeof value !== "object") return JSON.stringify(value)
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`
  const record = value as Record<string, unknown>
  return `{${Object.keys(record).sort().map(key => `${JSON.stringify(key)}:${canonical(record[key])}`).join(",")}}`
}

const sha = (bytes: string | Uint8Array): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(bytes).digest("hex")}`

describe("retained Agent Computer portable-session-control", () => {
  test("keeps stage non-accepting, activates the graph, checkpoints, and reclaims exactly once", async () => {
    const { stateRoot, calls, runtime } = await fixture()
    const stage = operation("stage", "operation.port03.guest.stage", { bundle, capabilityLeaseRefs: ["lease.port03.guest"] })
    const staged = await executePortableSessionControl({ operation: stage, stateRoot, runtime })
    expect(staged).toMatchObject({ acceptingWork: false })
    expect(await executePortableSessionControl({ operation: stage, stateRoot, runtime })).toEqual(staged)
    expect(calls).toEqual(["stage"])

    await expect(executePortableSessionControl({
      operation: operation("activate", "operation.port03.guest.activate.invalid", {
        authorityEvidenceRef: "evidence.port03.guest.authority",
        capabilityLeaseRefs: [],
      }),
      stateRoot,
      runtime,
    })).rejects.toThrow("capability")

    const activated = await executePortableSessionControl({
      operation: operation("activate", "operation.port03.guest.activate", {
        authorityEvidenceRef: "evidence.port03.guest.authority",
        capabilityLeaseRefs: ["lease.port03.guest"],
      }),
      stateRoot,
      runtime,
    })
    expect(activated).toMatchObject({
      activatedAgentRefs: ["agent.port03.guest.root", "agent.port03.guest.child"],
      acceptedWorkRefs: [],
    })
    await executePortableSessionControl({
      operation: operation("quiesce", "operation.port03.guest.quiesce", { graph, threadCursors: bundle.threadCursors }),
      stateRoot,
      runtime,
    })
    const checkpoint = await executePortableSessionControl({
      operation: operation("checkpoint", "operation.port03.guest.checkpoint", {
        checkpointRef: "checkpoint.port03.guest.managed",
        eventLogCursor: 10,
        executionBinding: bundle.executionBinding,
        graph,
        threadCursors: bundle.threadCursors,
      }),
      stateRoot,
      runtime,
    }) as { checkpoint: Record<string, unknown> }
    expect(checkpoint.checkpoint).toMatchObject({
      sourceGeneration: 2,
      repositoryRevisionRef: "e".repeat(40),
      repositoryPostImageDigest: `sha256:${"e".repeat(64)}`,
    })
    expect(checkpoint.checkpoint.digest).toMatch(/^sha256:[a-f0-9]{64}$/)
    const reclaimed = await executePortableSessionControl({
      operation: operation("reclaim", "operation.port03.guest.reclaim", {
        agentRefs: ["agent.port03.guest.root", "agent.port03.guest.child"],
      }),
      stateRoot,
      runtime,
    })
    expect(reclaimed).toMatchObject({ processes: "released", scratch: "released", ports: "released" })
    expect(calls).toEqual(["stage", "capabilities", "activate", "quiesce", "reclaim"])
  })

  test("rejects conflicting replay and private-shaped payloads", async () => {
    const { stateRoot, runtime } = await fixture()
    const stage = operation("stage", "operation.port03.guest.stage", { bundle })
    await executePortableSessionControl({ operation: stage, stateRoot, runtime })
    await expect(executePortableSessionControl({
      operation: { ...stage, payload: { bundle: { ...bundle, checkpoint: { ...bundle.checkpoint, diffDigest: `sha256:${"e".repeat(64)}` } } } },
      stateRoot,
      runtime,
    })).rejects.toThrow("conflict")
    await expect(executePortableSessionControl({
      operation: operation("checkpoint", "operation.port03.guest.unsafe", { password: "not-allowed" }),
      stateRoot,
      runtime,
    })).rejects.toBeInstanceOf(PortableSessionControlError)
  })

  test("abort reclaims a staged graph without activating it", async () => {
    const { stateRoot, calls, runtime } = await fixture()
    await executePortableSessionControl({
      operation: operation("stage", "operation.port03.guest.abort-stage", { bundle }),
      stateRoot,
      runtime,
    })
    const aborted = await executePortableSessionControl({
      operation: operation("abort", "operation.port03.guest.abort", {}),
      stateRoot,
      runtime,
    })
    expect(aborted).toHaveProperty("evidenceRefs")
    expect(calls).toEqual(["stage", "reclaim"])
  })

  test("executes one bounded turn per graph agent and fences replay", async () => {
    const { stateRoot, calls, runtime } = await fixture()
    const continuationGraph = {
      rootAgentRef: "agent.port03.guest.root",
      nodes: [
        { agentRef: "agent.port03.guest.root", threadRef: "thread.port03.guest.root", activityCursor: 3 },
        { agentRef: "agent.port03.guest.child", parentAgentRef: "agent.port03.guest.root", threadRef: "thread.port03.guest.child", activityCursor: 5 },
      ],
    }
    const continuationBundle = {
      ...bundle,
      executionBinding: { ...bundle.executionBinding, repositoryRef: "repository.OpenAgentsInc.openagents" },
      graph: continuationGraph,
      threadCursors: [
        { threadRef: "thread.port03.guest.root", transcriptRef: "transcript.port03.guest.root", activityCursor: 3, eventCursor: 8 },
        { threadRef: "thread.port03.guest.child", transcriptRef: "transcript.port03.guest.child", activityCursor: 5, eventCursor: 11 },
      ],
    }
    await executePortableSessionControl({
      operation: operation("stage", "operation.port03.guest.continue.stage", { bundle: continuationBundle, capabilityLeaseRefs: ["lease.port03.guest"] }),
      stateRoot,
      runtime,
    })
    await executePortableSessionControl({
      operation: operation("activate", "operation.port03.guest.continue.activate", {
        authorityEvidenceRef: "evidence.port03.guest.authority",
        capabilityLeaseRefs: ["lease.port03.guest"],
      }),
      stateRoot,
      runtime,
    })
    const continuation = {
      operationRef: "operation.port03.guest.continue",
      ownerRef: "owner.port03.guest",
      targetRef: "target.port03.guest.managed",
      sessionRef: "session.port03.guest",
      attachmentRef: "attachment.port03.guest.managed",
      generation: 2,
      providerLeaseRef: "lease.port03.guest",
      expectedThreadCursors: [
        { agentRef: "agent.port03.guest.root", threadRef: "thread.port03.guest.root", activityCursor: 3, eventCursor: 8 },
        { agentRef: "agent.port03.guest.child", threadRef: "thread.port03.guest.child", activityCursor: 5, eventCursor: 11 },
      ],
      turns: [
        { agentRef: "agent.port03.guest.root", turnRef: "turn.port03.guest.root", task: "Complete one bounded root turn." },
        { agentRef: "agent.port03.guest.child", turnRef: "turn.port03.guest.child", task: "Complete one bounded child turn." },
      ],
    }
    expect(await continuePortableSession({ continuation, stateRoot, runtime })).toMatchObject({
      acceptedWorkRefs: [
        { agentRef: "agent.port03.guest.root", turnRef: "turn.port03.guest.root" },
        { agentRef: "agent.port03.guest.child", turnRef: "turn.port03.guest.child" },
      ],
      replay: "executed",
      material: "excluded",
    })
    expect(await continuePortableSession({ continuation, stateRoot, runtime })).toMatchObject({ replay: "replayed" })
    expect(calls.filter(call => call === "continue")).toHaveLength(1)
    await expect(continuePortableSession({
      continuation: { ...continuation, turns: continuation.turns.map((turn, index) => index === 0 ? { ...turn, task: "changed" } : turn) },
      stateRoot,
      runtime,
    })).rejects.toThrow("operation bytes conflict")
  })

  test("installs raw capability material behind a refs-only marker and wipes both", async () => {
    const { stateRoot, runtime } = await fixture()
    await executePortableSessionControl({
      operation: operation("stage", "operation.port03.guest.capability-stage", { bundle }),
      stateRoot,
      runtime,
    })
    const material = new TextEncoder().encode("opaque-test-material")
    const metadata = {
      operationRef: "operation.port03.guest.capability-install",
      ownerRef: "owner.port03.guest",
      targetRef: "target.port03.guest.managed",
      resourceRef: "resource.port03.guest",
      sessionRef: "session.port03.guest",
      attachmentRef: "attachment.port03.guest.managed",
      generation: 2,
      leaseRef: "lease.port03.guest.provider",
      evidenceRef: "evidence.port03.guest.provider",
      capability: "capability.provider.codex",
    }
    const installed = await installPortableCapability({ metadata, material, stateRoot }) as Record<string, unknown>
    expect(installed).toMatchObject({
      evidenceRef: metadata.evidenceRef,
      marker: { leaseRef: metadata.leaseRef, evidenceRef: metadata.evidenceRef },
      material: "excluded",
    })
    const sessionRoot = portableSessionRoot(stateRoot, metadata.sessionRef)
    const leaf = new Bun.CryptoHasher("sha256").update(metadata.leaseRef).digest("hex").slice(0, 24)
    expect(await readFile(join(sessionRoot, "capability-material", `${leaf}.material`), "utf8")).toBe("opaque-test-material")

    const wiped = await executePortableSessionControl({
      operation: operation("wipeCapability", "operation.port03.guest.capability-wipe", {
        leaseRef: metadata.leaseRef,
        installationRef: installed.installationRef,
      }),
      stateRoot,
      runtime,
    })
    expect(wiped).toMatchObject({ material: "excluded" })
    expect(await Bun.file(join(sessionRoot, "capability-material", `${leaf}.material`)).exists()).toBe(false)
    expect(await Bun.file(join(sessionRoot, "capabilities", `${leaf}.installed.json`)).exists()).toBe(false)
  })

  test("safely materializes a private checkpoint archive before verified stage", async () => {
    const stateRoot = await mkdtemp(join(tmpdir(), "portable-materialize-state-"))
    const sourceRoot = await mkdtemp(join(tmpdir(), "portable-materialize-source-"))
    const archiveRoot = await mkdtemp(join(tmpdir(), "portable-materialize-archive-"))
    roots.push(stateRoot, sourceRoot, archiveRoot)
    const sourceWorkspace = join(sourceRoot, "workspace")
    await mkdir(sourceWorkspace)
    await run(["git", "init", "--quiet"], sourceWorkspace)
    await run(["git", "config", "user.email", "portable@example.invalid"], sourceWorkspace)
    await run(["git", "config", "user.name", "Portable Test"], sourceWorkspace)
    await writeFile(join(sourceWorkspace, "tracked.txt"), "base\n")
    await run(["git", "add", "tracked.txt"], sourceWorkspace)
    await run(["git", "commit", "--quiet", "-m", "base"], sourceWorkspace)
    await writeFile(join(sourceWorkspace, "tracked.txt"), "changed\n")
    await writeFile(join(sourceWorkspace, "untracked.txt"), "new\n")
    await symlink("tracked.txt", join(sourceWorkspace, "CLAUDE.md"))
    const snapshot = await repositorySnapshot(sourceRoot)
    const bundlePath = join(archiveRoot, "repository.bundle")
    await run(["git", "bundle", "create", bundlePath, "HEAD"], sourceWorkspace)
    const postImage = join(archiveRoot, "post-image")
    await mkdir(postImage)
    await cp(join(sourceWorkspace, "tracked.txt"), join(postImage, "tracked.txt"))
    await cp(join(sourceWorkspace, "untracked.txt"), join(postImage, "untracked.txt"))
    await symlink("tracked.txt", join(postImage, "CLAUDE.md"))
    const files = await Promise.all(["CLAUDE.md", "tracked.txt", "untracked.txt"].map(async path => {
      if (path === "CLAUDE.md") {
        const linkTarget = await readlink(join(postImage, path))
        const bytes = new TextEncoder().encode(linkTarget)
        return { path, mode: 0o120000, sha256: sha(bytes), size: bytes.byteLength, linkTarget }
      }
      const bytes = await readFile(join(postImage, path))
      return { path, mode: 0o644, sha256: sha(bytes), size: bytes.byteLength }
    }))
    const normalizedGraph = { ...graph, nodes: [...graph.nodes].sort((a, b) => a.agentRef.localeCompare(b.agentRef)) }
    const materializedBundle = {
      ...bundle,
      checkpoint: {
        ...bundle.checkpoint,
        checkpointRef: "checkpoint.port03.guest.source",
        ...snapshot,
        graphDigest: sha(canonical(normalizedGraph)),
      },
    }
    const artifactRef = "artifact.port03.guest.private"
    await writeFile(join(archiveRoot, "manifest.json"), JSON.stringify({
      schema: "openagents.portable_checkpoint_artifact.v1",
      artifactRef,
      checkpointRef: materializedBundle.checkpoint.checkpointRef,
      bundle: materializedBundle,
      files,
    }))
    const tarPath = join(archiveRoot, "checkpoint.tar")
    const zstdPath = join(archiveRoot, "checkpoint.tar.zst")
    await run(["tar", "-C", archiveRoot, "-cf", tarPath, "manifest.json", "repository.bundle", "post-image"])
    await run(["zstd", "-q", "-f", tarPath, "-o", zstdPath])
    const archive = await readFile(zstdPath)
    const stage = operation("stage", "operation.port03.guest.materialized-stage", {
      bundle: materializedBundle,
      capabilityLeaseRefs: ["lease.port03.guest.provider"],
    })
    const preparedRuntime: PortableSessionGuestRuntime = {
      ...productionRuntime,
      activate: async () => undefined,
      quiesce: async () => undefined,
      verifyCapabilities: async () => undefined,
      prepare: async ({ sessionRoot, agentRefs }) => {
        for (const agentRef of agentRefs) {
          const leaf = createHash("sha256").update(agentRef).digest("hex").slice(0, 24)
          const dir = join(sessionRoot, "agents", leaf)
          await mkdir(dir, { recursive: true })
          await writeFile(join(dir, "lifecycle-state.json"), "{}")
        }
      },
    }
    const receipt = await materializePortableCheckpoint({
      metadata: {
        operationRef: "operation.port03.guest.materialize",
        artifactRef,
        artifactDigest: sha(archive),
        stageOperation: stage,
      },
      archive,
      stateRoot,
      runtime: preparedRuntime,
      zstdBin: "/opt/homebrew/bin/zstd",
    })
    expect(receipt).toMatchObject({ acceptingWork: false })
    expect(await readlink(join(portableSessionRoot(stateRoot, stage.sessionRef), "workspace", "CLAUDE.md"))).toBe("tracked.txt")
    expect(await repositorySnapshot(portableSessionRoot(stateRoot, stage.sessionRef))).toEqual(snapshot)

    await executePortableSessionControl({
      operation: operation("activate", "operation.port03.guest.materialized-activate", {
        authorityEvidenceRef: "evidence.port03.guest.materialized-authority",
        capabilityLeaseRefs: ["lease.port03.guest.provider"],
      }),
      stateRoot,
      runtime: preparedRuntime,
    })
    await executePortableSessionControl({
      operation: operation("quiesce", "operation.port03.guest.materialized-quiesce", { graph }),
      stateRoot,
      runtime: preparedRuntime,
    })
    const exportedCheckpointRef = "checkpoint.port03.guest.exported"
    await executePortableSessionControl({
      operation: operation("checkpoint", "operation.port03.guest.materialized-checkpoint", {
        checkpointRef: exportedCheckpointRef,
        eventLogCursor: 10,
        executionBinding: materializedBundle.executionBinding,
        graph: materializedBundle.graph,
        threadCursors: materializedBundle.threadCursors,
      }),
      stateRoot,
      runtime: preparedRuntime,
    })
    const exportMetadata = {
      operationRef: "operation.port03.guest.checkpoint-export",
      ownerRef: stage.ownerRef,
      targetRef: stage.targetRef,
      sessionRef: stage.sessionRef,
      attachmentRef: stage.attachmentRef,
      generation: stage.generation,
      checkpointRef: exportedCheckpointRef,
    }
    const exported = await exportPortableCheckpoint({
      metadata: exportMetadata,
      stateRoot,
      zstdBin: "/opt/homebrew/bin/zstd",
    }) as Record<string, unknown>
    expect(exported).toMatchObject({ checkpointRef: exportedCheckpointRef, material: "excluded" })
    expect(await exportPortableCheckpoint({ metadata: exportMetadata, stateRoot, zstdBin: "/opt/homebrew/bin/zstd" })).toEqual(exported)
    const exportedRoot = join(
      portableSessionRoot(stateRoot, stage.sessionRef),
      "exports",
      createHash("sha256").update(exportMetadata.operationRef).digest("hex").slice(0, 24),
    )
    expect(sha(await readFile(join(exportedRoot, "checkpoint.tar.zst")))).toBe(exported.artifactDigest)
  }, 15_000)
})
